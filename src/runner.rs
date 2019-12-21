use {
    std::collections::HashMap,
    super::windowmanager::WindowManager,
    super::callbacks::*,
    super::xlibwrapper::action::Action,
    super::xlibwrapper::core::*,
    std::rc::Rc,
    std::sync::mpsc,
};

pub struct Runner {
    call_table: HashMap<i32, Callback>,
    lib: Rc<XlibWrapper>,
    wm: WindowManager
}

impl Runner {
    pub fn new(lib: Rc<XlibWrapper>, wm: WindowManager) -> Self {
        let mut ret = Self {
            call_table: HashMap::new(),
            lib,
            wm
        };

        ret.call_table.insert(xlib::MapRequest, Box::new(map_request::map_request));
        ret.call_table.insert(xlib::UnmapNotify, Box::new(unmap_notify::unmap_notify));
        ret.call_table.insert(xlib::ConfigureRequest, Box::new(configure_request::configure_request));
        ret.call_table.insert(xlib::ClientMessage, Box::new(client_message_request::client_message_request));
        ret.call_table.insert(xlib::MotionNotify, Box::new(motion_notify::motion_notify));
        ret.call_table.insert(xlib::DestroyNotify, Box::new(destroy_window::destroy_window));
        ret.call_table.insert(xlib::Expose, Box::new(expose::expose));
        ret.call_table.insert(xlib::LeaveNotify, Box::new(leave_notify::leave_notify));
        ret.call_table.insert(xlib::EnterNotify, Box::new(enter_notify::enter_notify));
        ret.call_table.insert(xlib::ButtonPress, Box::new(button_press::button_press));
        ret.call_table.insert(xlib::KeyPress, Box::new(key_press::key_press));
        ret.call_table.insert(xlib::KeyRelease, Box::new(key_release::key_release));
        ret.call_table.insert(xlib::ButtonRelease, Box::new(button_release::button_release));
        ret.call_table.insert(xlib::PropertyNotify, Box::new(property_notify::property_notify));


        ret
    }


    pub fn run(&mut self, tx: mpsc::Sender<bool>) {

        self.lib.grab_server();
        let _ = self.lib.get_top_level_windows()
            .iter()
            .map(|w| {
                self.wm.setup_window(*w)
            });
        self.lib.ungrab_server();

        let _ = tx.send(true);

        loop {
            let event = self.lib.next_event();

            match self.call_table.get(&event.get_type()) {
                Some(func) => func(self.lib.clone(), &mut self.wm, Action::from(event)),
                None => { continue; }
            }
        }
    }
}
