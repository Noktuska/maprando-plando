use egui::Ui;
use strum::VariantArray;

pub trait SettingsGen {
    fn generate<S: Into<String>>(&mut self, label: S, ui: &mut Ui);
}

impl SettingsGen for bool {
    fn generate<S: Into<String>>(&mut self, label: S, ui: &mut Ui) {
        ui.label(label.into());
        ui.selectable_value(self, false, "No");
        ui.selectable_value(self, true, "Yes");
        ui.end_row();
    }
}

pub trait SettingsPreset<K: VariantArray + PartialEq + ToString> where Self: Sized + PartialEq {
    fn generate<S: Into<String>>(&mut self, label: S, ui: &mut Ui) {
        ui.label(label.into());

        for var in K::VARIANTS {
            let preset = Self::get(var);
            if ui.selectable_label(*self == preset, var.to_string()).clicked() {
                *self = preset;
            }
        }
    }

    fn get(key: &K) -> Self;
}