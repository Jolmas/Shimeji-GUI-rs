use std::sync::mpsc;
use ksni::blocking::TrayMethods;

#[derive(Debug, Clone)]
pub enum TrayEvent { Show, Quit, Summon, DismissAll, Settings }

#[derive(Clone, Debug)]
pub struct TrayLabels {
    pub show: String,
    pub summon: String,
    pub dismiss_all: String,
    pub settings: String,
    pub quit: String,
}

impl Default for TrayLabels {
    fn default() -> Self {
        Self {
            show: "Show".into(),
            summon: "Summon".into(),
            dismiss_all: "Dismiss all".into(),
            settings: "Settings".into(),
            quit: "Quit".into(),
        }
    }
}

#[derive(Clone)]
struct Tray {
    tx: mpsc::Sender<TrayEvent>,
    labels: TrayLabels,
}

impl ksni::Tray for Tray {
    fn icon_name(&self) -> String { "applications-games".into() }
    fn title(&self) -> String { "wl_shimeji GUI".into() }
    fn id(&self) -> String { "shimeji-gui-rs".into() }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let l = self.labels.clone();
        vec![
            StandardItem { label: l.show, activate: Box::new(|t: &mut Self| { let _ = t.tx.send(TrayEvent::Show); }), ..Default::default() }.into(),
            StandardItem { label: l.summon, activate: Box::new(|t: &mut Self| { let _ = t.tx.send(TrayEvent::Summon); }), ..Default::default() }.into(),
            StandardItem { label: l.dismiss_all, activate: Box::new(|t: &mut Self| { let _ = t.tx.send(TrayEvent::DismissAll); }), ..Default::default() }.into(),
            StandardItem { label: l.settings, activate: Box::new(|t: &mut Self| { let _ = t.tx.send(TrayEvent::Settings); }), ..Default::default() }.into(),
            MenuItem::Separator,
            StandardItem { label: l.quit, activate: Box::new(|t: &mut Self| { let _ = t.tx.send(TrayEvent::Quit); }), ..Default::default() }.into(),
        ]
    }
}

pub struct TrayControl {
    handle: ksni::blocking::Handle<Tray>,
}

impl TrayControl {
    #[allow(dead_code)]
    pub fn update_labels(&self, labels: TrayLabels) {
        let _ = self.handle.update(move |tray: &mut Tray| {
            tray.labels = labels;
        });
    }
}

pub fn spawn_tray(labels: TrayLabels) -> Option<(mpsc::Receiver<TrayEvent>, TrayControl)> {
    let (tx, rx) = mpsc::channel();
    let tray = Tray { tx, labels };
    match tray.disable_dbus_name(false).assume_sni_available(true).spawn() {
        Ok(handle) => Some((rx, TrayControl { handle })),
        Err(_) => None,
    }
}
