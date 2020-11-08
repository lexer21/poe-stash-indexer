use std::fmt::Display;

#[derive(Debug, Clone)]
pub struct ChangeID {
    pub(crate) inner: String,
}

impl Display for ChangeID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.inner)
    }
}

impl std::str::FromStr for ChangeID {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let is_valid = s.split('-').map(|x| x.parse::<u32>()).all(|x| x.is_ok());

        match is_valid {
            true => Ok(Self {
                inner: s.to_owned(),
            }),
            false => Err("derp".into()),
        }
    }
}

impl Into<String> for ChangeID {
    fn into(self) -> String {
        self.inner
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_from_str_success() {
        let change_id = ChangeID::from_str("850662131-863318628-825558626-931433265-890834941");

        assert!(change_id.is_ok(),);
        assert_eq!(
            change_id.unwrap().inner,
            "850662131-863318628-825558626-931433265-890834941"
        );
    }

    #[test]
    fn test_from_str_err() {
        assert!(
            super::ChangeID::from_str("850662A31-863318628-825558626-931433265-890834941").is_err(),
        );
    }
}
