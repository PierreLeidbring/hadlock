use crate::{layout::LayoutTag, state::State, wm, xlibwrapper::action};
use reducer::*;

impl Reducer<action::UnmapNotify> for State {
    fn reduce(&mut self, action: action::UnmapNotify) {
        //debug!("UnmapNotify");
        let mon_id = wm::get_mon_by_window(&self, action.win).unwrap_or(self.current_monitor);
        let mon = self
            .monitors
            .get_mut(&mon_id)
            .expect("No monitor was found?!");
        mon.remove_window(action.win);
        if mon.get_current_layout() != LayoutTag::Floating {
            wm::reorder(self);
        }
    }
}
