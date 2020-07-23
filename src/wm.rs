use crate::{
    config::Axis,
    layout::LayoutTag,
    models::{monitor::Monitor, rect::*, screen::*, windowwrapper::*, workspace::*, WindowState},
    state::State,
    xlibwrapper::xlibmodels::*,
};

pub fn window_inside_screen(w_geom: &Geometry, screen: &Screen) -> bool {
    let inside_width = w_geom.x >= screen.x && w_geom.x < screen.x + screen.width;
    let inside_height = w_geom.y >= screen.y && w_geom.y < screen.y + screen.height;
    inside_width && inside_height
}

#[allow(clippy::collapsible_if)]
pub fn toggle_maximize(mon: &Monitor, ww: WindowWrapper) -> WindowWrapper {
    if mon.get_current_layout() != LayoutTag::Floating {
        if mon.get_current_ws().unwrap().clients.is_empty() {
            return ww;
        }
    }
    let ww_state = ww.current_state;
    match ww_state {
        WindowState::Maximized => WindowWrapper {
            window_rect: Rect::new(ww.restore_position, ww.restore_size),
            previous_state: ww.current_state,
            current_state: ww.previous_state,
            ..ww
        },
        _ => {
            let (pos, size) = mon.maximize(ww.window(), &ww);
            WindowWrapper {
                restore_position: ww.get_position(),
                restore_size: ww.get_size(),
                window_rect: Rect::new(pos, size),
                previous_state: ww.current_state,
                current_state: WindowState::Maximized,
                ..ww
            }
        }
    }
}

pub fn toggle_monocle(mon: &Monitor, ww: WindowWrapper) -> WindowWrapper {
    let ww_state = ww.current_state;
    match ww_state {
        WindowState::Monocle => WindowWrapper {
            window_rect: Rect::new(ww.restore_position, ww.restore_size),
            previous_state: ww.current_state,
            current_state: ww.previous_state,
            ..ww
        },
        _ => {
            let (pos, size) = mon.monocle(ww.window(), &ww);
            WindowWrapper {
                restore_position: ww.get_position(),
                restore_size: ww.get_size(),
                window_rect: Rect::new(pos, size),
                previous_state: ww.current_state,
                current_state: WindowState::Monocle,
                ..ww
            }
        }
    }
}

pub fn get_mon_by_ws(state: &State, ws: u32) -> Option<MonitorId> {
    let mut ret_vec = state
        .monitors
        .iter()
        .filter(|(_, val)| val.contains_ws(ws))
        .map(|(key, _)| *key)
        .collect::<Vec<MonitorId>>();
    if ret_vec.len() != 1 {
        None
    } else {
        Some(ret_vec.remove(0))
    }
}

pub fn get_mon_by_window(state: &State, w: Window) -> Option<MonitorId> {
    let mut ret_vec = state
        .monitors
        .iter()
        .filter(|(_, val)| val.contains_window(w))
        .map(|(key, _)| *key)
        .collect::<Vec<MonitorId>>();
    if ret_vec.len() != 1 {
        None
    } else {
        Some(ret_vec.remove(0))
    }
}

pub fn resize_window(state: &mut State, w: Window, axis: &Axis, delta: i32) {
    let mon = state.monitors.get_mut(&state.current_monitor).unwrap();
    let windows = mon.resize_window(w, axis, delta);
    windows.into_iter().for_each(|ww| {
        mon.swap_window(ww.window(), |_, _| ww.clone());
    });
}

pub fn set_current_ws(state: &mut State, ws: u32) -> Option<()> {
    let mon = state.monitors.get_mut(&state.current_monitor)?;
    if mon.id == state.current_monitor && ws == mon.current_ws {
        return Some(());
    }

    let mon = match get_mon_by_ws(state, ws) {
        Some(mon) => state.monitors.get_mut(&mon)?,
        None => state.monitors.get_mut(&state.current_monitor)?,
    };
    debug!("Mon: {} current_ws: {}", mon.id, mon.current_ws);
    if ws == mon.current_ws {
        let mon = state.monitors.get_mut(&state.current_monitor)?;

        let curr_ws = mon.get_current_ws_mut()?;
        curr_ws.focus_w = curr_ws
            .get_newest()
            .map(|(win, _)| *win)
            .unwrap_or(state.lib.get_root());

        let mon = state.monitors.get_mut(&get_mon_by_ws(state, ws)?)?;

        let newest = mon.get_current_ws()?.focus_w;

        state.focus_w = newest;
        state.current_monitor = mon.id;
        return Some(());
    }
    mon.swap_ws(mon.current_ws, |_, mut ws| {
        ws.apply_to_all(|ww| {
            ww.hidden = true;
        });
        ws
    });

    if mon.contains_ws(ws) {
        let mut new_ws = mon.remove_ws(ws)?;
        new_ws.apply_to_all(|ww| {
            ww.hidden = false;
            ww.set_position(ww.get_position());
        });
        let win = new_ws.focus_w;
        debug!(
            "new_ws clients: {:?}",
            new_ws.clients.keys().collect::<Vec<&Window>>()
        );
        state.focus_w = new_ws.focus_w;
        mon.add_ws(new_ws);
        debug!("new_ws.focus_w: 0x{:x}", win);
    } else {
        mon.add_ws(Workspace::new(ws, state.lib.get_root()));
    }

    state.current_monitor = mon.id;
    if mon.workspaces.get(&mon.current_ws)?.clients.is_empty() && !mon.workspaces.is_empty() {
        mon.remove_ws(mon.current_ws);
    }
    mon.current_ws = ws;
    state.current_monitor = mon.id;
    state.latest_cursor_pos = state.lib.pointer_pos(state.lib.get_root());
    Some(())
}

pub fn move_to_ws(state: &mut State, w: Window, ws: u32) -> Option<()> {
    if ws == state.monitors.get(&state.current_monitor)?.current_ws {
        return Some(());
    }

    let ww = state
        .monitors
        .get_mut(&state.current_monitor)?
        .remove_window(w)?;

    let mon = match get_mon_by_ws(state, ws) {
        Some(mon) => state.monitors.get_mut(&mon)?,
        None => state.monitors.get_mut(&state.current_monitor)?,
    };

    if ws == mon.current_ws {
        debug!("is current");
        let prev_ws = mon.current_ws;
        mon.current_ws = ws;
        let windows = mon.place_window(w);
        let current_state = if windows.len() == 1 && mon.get_current_layout() != LayoutTag::Floating
        {
            WindowState::Maximized
        } else {
            WindowState::Free
        };
        mon.add_window(w, ww.clone());
        for (win, rect) in windows.into_iter() {
            let (new_win, new_ww) = mon.get_newest()?;
            let new_win = *new_win;
            let new_ww = new_ww.clone();
            mon.swap_window(win, |_mon, ww| WindowWrapper {
                restore_position: rect.get_position(),
                window_rect: rect,
                previous_state: WindowState::Free,
                current_state,
                toc: if win == w { new_ww.toc } else { ww.toc },
                ..ww
            });
            if win == w {
                mon.swap_window(new_win, |_mon, win_wrap| WindowWrapper {
                    current_state,
                    toc: ww.toc,
                    ..win_wrap
                });
            }
        }
        mon.get_current_ws_mut().unwrap().focus_w = *mon.get_newest().unwrap().0;
        mon.current_ws = prev_ws;
        if let (Some(newest), Some(ws)) = (
            mon.get_newest().map(|(newest, _)| newest).copied(),
            mon.get_current_ws_mut(),
        ) {
            debug!("heeeey you!");
            ws.focus_w = newest;
        }
        return Some(());
    }

    if mon.contains_ws(ws) {
        let prev_ws = mon.current_ws;
        mon.current_ws = ws;
        mon.add_window(w, ww.clone());
        let windows = mon.place_window(w);
        let current_state = if windows.len() == 1 && mon.get_current_layout() != LayoutTag::Floating
        {
            WindowState::Maximized
        } else {
            WindowState::Free
        };
        for (win, rect) in windows.into_iter() {
            let (new_win, new_ww) = mon.get_newest().clone().unwrap();
            let new_win = *new_win;
            let new_ww = new_ww.clone();
            mon.swap_window(win, |_mon, ww| WindowWrapper {
                restore_position: rect.get_position(),
                window_rect: rect,
                previous_state: ww.previous_state,
                current_state,
                toc: if win == w { new_ww.toc } else { ww.toc },
                ..ww
            });
            if win == w {
                mon.swap_window(new_win, |_mon, win_wrap| WindowWrapper {
                    current_state,
                    toc: ww.toc,
                    ..win_wrap
                });
            }
        }
        mon.get_current_ws_mut().unwrap().focus_w = *mon.get_newest().unwrap().0;
        mon.current_ws = prev_ws;
        if let (Some(newest), Some(ws)) = (
            mon.get_newest().map(|(newest, _)| newest).copied(),
            mon.get_current_ws_mut(),
        ) {
            debug!("heeeey you!");
            ws.focus_w = newest;
        }
    } else {
        let prev_ws = mon.current_ws;
        mon.current_ws = ws;
        mon.add_ws(Workspace::new(ws, state.lib.get_root()));
        let windows = mon.place_window(w);
        let current_state = if windows.len() == 1 && mon.get_current_layout() != LayoutTag::Floating
        {
            WindowState::Maximized
        } else {
            WindowState::Free
        };

        windows.into_iter().for_each(|(win, rect)| {
            let new_ww = WindowWrapper {
                restore_position: rect.get_position(),
                previous_state: WindowState::Free,
                current_state,
                window_rect: rect,
                ..ww
            };
            mon.add_window(win, new_ww);
        });
        mon.get_current_ws_mut().unwrap().focus_w = *mon.get_newest().unwrap().0;
        mon.current_ws = prev_ws;
        if let (Some(newest), Some(ws)) = (
            mon.get_newest().map(|(newest, _)| *newest),
            mon.get_current_ws_mut(),
        ) {
            debug!("hola compadre");
            ws.focus_w = newest;
        }
    }

    Some(())
}

pub fn reorder(state: &mut State) -> Option<()> {
    let mon = state.monitors.get_mut(&state.current_monitor)?;
    debug!("reorder focus: {}", state.focus_w);
    let windows = mon
        .get_current_ws()?
        .clients
        .values()
        .filter(|ww| !ww.is_trans)
        .cloned()
        .collect::<Vec<WindowWrapper>>();

    if state.focus_w == state.lib.get_root() {
        debug!("reorder focus is root");
        return None;
    }

    let rects = mon.reorder(state.focus_w, &windows);

    let current_state = if mon.get_current_layout() == LayoutTag::Floating {
        WindowState::Free
    } else if rects.len() == 1 {
        WindowState::Maximized
    } else {
        WindowState::Tiled
    };

    for (win, rect) in rects {
        mon.swap_window(win, |_mon, ww| WindowWrapper {
            window_rect: rect,
            current_state,
            ..ww
        });
    }
    debug!("return from reorder");
    Some(())
}

pub fn point_is_inside(_state: &State, screen: &Screen, x: i32, y: i32) -> bool {
    let inside_height = y >= screen.y && y <= screen.y + screen.height as i32;

    let inside_width = x >= screen.x && x <= screen.x + screen.width as i32;

    inside_height && inside_width
}

pub fn get_monitor_by_point(state: &State, x: i32, y: i32) -> MonitorId {
    let mon_vec = state
        .monitors
        .iter()
        .filter(|(_key, mon)| point_is_inside(state, &mon.screen, x, y))
        .map(|(key, _mon)| *key)
        .collect::<Vec<u32>>();
    match mon_vec.get(0) {
        Some(mon_id) => *mon_id,
        None => state.current_monitor,
    }
}

#[cfg(test)]
mod test {
    use crate::models::{
        monitor::Monitor, rect::Rect, screen::Screen, windowwrapper::WindowWrapper,
        workspace::Workspace, WindowState,
    };
    use crate::wm;
    use crate::xlibwrapper::xlibmodels::{Geometry, Window};

    const ROOT: Window = 1;
    const SCREEN_1: Screen = Screen {
        root: ROOT,
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    const WIN_GEOM: Geometry = Geometry {
        width: 100,
        height: 100,
        x: 10,
        y: 10,
    };

    #[test]
    fn window_inside_screen_pass() {
        assert!(wm::window_inside_screen(&WIN_GEOM, &SCREEN_1))
    }

    #[test]
    fn window_inside_screen_fail() {
        let win_geom = Geometry {
            width: 100,
            height: 100,
            x: -10,
            y: -10,
        };
        assert_ne!(true, wm::window_inside_screen(&win_geom, &SCREEN_1))
    }
    // pub fn toggle_maximize(mon: &Monitor, ww: WindowWrapper) -> WindowWrapper;

    #[test]
    fn toggle_maximize_to_maximized() {
        let ws = Workspace::new(1, 0);
        let mon = Monitor::new(1, SCREEN_1, ws);

        let original = WindowWrapper::new(12, Rect::from(WIN_GEOM), false);

        let tested = wm::toggle_maximize(&mon, original.clone());

        assert_eq!(
            WindowState::Maximized,
            tested.current_state,
            "Wrong state: {:?}, should have been {:?}",
            tested.current_state,
            WindowState::Maximized
        );
        assert_eq!(
            original.current_state, tested.previous_state,
            "{:?}, should have been {:?}",
            tested.previous_state, original.current_state
        );
        assert_eq!(
            original.get_position(),
            tested.restore_position,
            "{:?}, should have been: {:?}",
            tested.restore_position,
            original.get_position()
        );
        assert_eq!(
            original.get_size(),
            tested.restore_size,
            "{:?}, should have been {:?}",
            tested.restore_size,
            original.get_size()
        );
    }
}
