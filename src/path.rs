use semver::Version;
use snafu::{ResultExt, Snafu};
use std::str::FromStr;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ForeignInterfacePath {
    package_name: String,
    interface_name: String,
    version: Option<Version>,
}

impl ForeignInterfacePath {
    #[must_use]
    pub fn new(package_name: String, interface_name: String, version: Option<Version>) -> Self {
        ForeignInterfacePath {
            package_name,
            interface_name,
            version,
        }
    }

    #[must_use]
    pub fn package_name(&self) -> &str {
        self.package_name.as_ref()
    }

    #[must_use]
    pub fn interface_name(&self) -> &str {
        &self.interface_name
    }

    #[must_use]
    pub fn version(&self) -> Option<&Version> {
        self.version.as_ref()
    }
}

impl From<ForeignInterfacePath> for InterfacePath {
    fn from(path: ForeignInterfacePath) -> Self {
        InterfacePath {
            package_name: Some(path.package_name),
            interface_name: path.interface_name,
            version: path.version,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct InterfacePath {
    package_name: Option<String>,
    interface_name: String,
    version: Option<Version>,
}

impl InterfacePath {
    #[must_use]
    pub fn new(
        package_name: Option<String>,
        interface_name: String,
        version: Option<Version>,
    ) -> Self {
        InterfacePath {
            package_name,
            interface_name,
            version,
        }
    }

    #[must_use]
    pub fn package_name(&self) -> Option<&str> {
        self.package_name.as_deref()
    }

    #[must_use]
    pub fn interface_name(&self) -> &str {
        &self.interface_name
    }

    #[must_use]
    pub fn version(&self) -> Option<&Version> {
        self.version.as_ref()
    }

    #[must_use]
    pub fn into_foreign(self) -> Option<ForeignInterfacePath> {
        Some(ForeignInterfacePath {
            package_name: self.package_name?,
            interface_name: self.interface_name,
            version: self.version,
        })
    }
}

impl FromStr for InterfacePath {
    type Err = InterfacePathParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parses the following format: "package_name/interface_name@version",
        // where the version specifier is optional.

        let parts: Vec<&str> = s.split('/').collect();

        match parts.len() {
            1 if s.contains('@') => return Err(InterfacePathParseError::FormatError),
            1 => {
                return Ok(Self {
                    package_name: None,
                    interface_name: s.to_string(),
                    version: None,
                });
            }
            2 => (), // Continue below.
            _ => return Err(InterfacePathParseError::FormatError),
        }

        let package_name = parts[0].to_string();

        let interface_parts: Vec<&str> = parts[1].split('@').collect();
        let interface_name = interface_parts[0].to_string();

        let version = if interface_parts.len() == 2 {
            Some(
                Version::parse(interface_parts[1])
                    .context(interface_path_parse_error::VersionParseSnafu)?,
            )
        } else {
            None
        };

        Ok(InterfacePath {
            package_name: Some(package_name),
            interface_name,
            version,
        })
    }
}

#[derive(Snafu, Debug)]
#[snafu(module)]
pub enum InterfacePathParseError {
    FormatError,
    VersionParseError { source: semver::Error },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interface_path_parsing() {
        let path = InterfacePath::from_str("package_name/interface_name@1.0.0").unwrap();
        assert_eq!(path.package_name(), Some("package_name"));
        assert_eq!(path.interface_name(), "interface_name");
        assert_eq!(path.version(), Some(&Version::parse("1.0.0").unwrap()));

        let path = InterfacePath::from_str("interface_name").unwrap();
        assert_eq!(path.package_name(), None);
        assert_eq!(path.interface_name(), "interface_name");
        assert_eq!(path.version(), None);

        let path = InterfacePath::from_str("package_name/interface_name").unwrap();
        assert_eq!(path.package_name(), Some("package_name"));
        assert_eq!(path.interface_name(), "interface_name");
        assert_eq!(path.version(), None);

        let path_err = InterfacePath::from_str("package_name/interface_name/").unwrap_err();
        assert!(matches!(path_err, InterfacePathParseError::FormatError));

        let path_err = InterfacePath::from_str("package_name/interface_name@").unwrap_err();
        assert!(matches!(
            path_err,
            InterfacePathParseError::VersionParseError { .. }
        ));
    }
}
