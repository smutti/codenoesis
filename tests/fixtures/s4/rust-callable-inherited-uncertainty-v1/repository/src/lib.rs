pub fn known(value: u8) -> u8 {
    value
}

#[cfg(feature = "gated")]
pub fn gated() -> u8 {
    1
}

#[cfg(test)]
mod tests {
    fn hidden_test() -> u8 {
        super::known(1)
    }
}

pub struct Local;

pub trait LocalTrait {
    fn local(&self) -> u8;
}

impl Local {
    pub fn inherent(&self) -> u8 {
        known(1)
    }
}

impl LocalTrait for Local {
    fn local(&self) -> u8 {
        self.inherent()
    }
}

#[cfg(feature = "gated")]
impl ExternalTrait for Local {
    fn unresolved_external_trait(&self) {}
}

#[cfg(feature = "gated")]
impl LocalTrait for ImportedType {
    fn unresolved_external_target(&self) -> u8 {
        0
    }
}
