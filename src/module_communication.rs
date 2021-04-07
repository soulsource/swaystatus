pub enum MsgMainToModule {
    Quit,
}
pub enum MsgModuleToMain {
    UpdateText {
        text : Result<String, String>
    }
}

pub trait SwayStatusModule {
    fn new(from_main : crossbeam_channel::Receiver<MsgMainToModule>,
            to_main : crossbeam_channel::Sender<MsgModuleToMain>,
            module_settings : &str) -> Result<Box<Self>,String>;

    fn get_name(&self) -> &'static str;

    fn run(&self);
}
