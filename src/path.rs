use semver::Version;
use snafu::{ResultExt, Snafu};
use std::fmt::Display;
use std::str::FromStr;

/// A fully-qualified path to a WIT interface, with an optional version.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ForeignInterfacePath {
    package_name: String,
    interface_name: String,
    version: Option<Version>,
}

impl ForeignInterfacePath {
    /// Creates a new `ForeignInterfacePath` with the given package name, interface name, and optional version.
    #[must_use]
    pub const fn new(
        package_name: String,
        interface_name: String,
        version: Option<Version>,
    ) -> Self {
        ForeignInterfacePath {
            package_name,
            interface_name,
            version,
        }
    }

    /// Returns the package name component of the interface path.
    #[must_use]
    pub fn package_name(&self) -> &str {
        self.package_name.as_ref()
    }

    /// Returns the interface name component of the interface path.
    #[must_use]
    pub fn interface_name(&self) -> &str {
        &self.interface_name
    }

    /// Returns the version component of the interface path, if one is specified.
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

impl Display for ForeignInterfacePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}/{}{}",
            self.package_name,
            self.interface_name,
            self.version
                .as_ref()
                .map_or(String::new(), |v| format!("@{v}"))
        )
    }
}

/// Represents a path to a WIT interface, which may be local (without a package name) or foreign
/// (with a package name). The version is optional in both cases.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct InterfacePath {
    package_name: Option<String>,
    interface_name: String,
    version: Option<Version>,
}

impl InterfacePath {
    #[must_use]
    pub const fn new(
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

    /// Returns the package name component of the interface path, if one is specified.
    #[must_use]
    pub fn package_name(&self) -> Option<&str> {
        self.package_name.as_deref()
    }

    /// Returns the interface name component of the interface path.
    #[must_use]
    pub fn interface_name(&self) -> &str {
        &self.interface_name
    }

    /// Returns the version component of the interface path, if one is specified.
    #[must_use]
    pub fn version(&self) -> Option<&Version> {
        self.version.as_ref()
    }

    /// Converts this `InterfacePath` into a `ForeignInterfacePath`, if it has a package name,
    /// otherwise returns `None`.
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

impl Display for InterfacePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.package_name
                .as_ref()
                .map_or(String::new(), |p| format!("{p}/")),
            self.interface_name,
            self.version
                .as_ref()
                .map_or(String::new(), |v| format!("@{v}")),
        )
    }
}

#[derive(Snafu, Debug)]
#[snafu(module)]
pub enum InterfacePathParseError {
    #[snafu(display("Invalid interface path format"))]
    FormatError,

    #[snafu(display("Invalid semantic version format: {}", source))]
    VersionParseError { source: semver::Error },
}

#[cfg(test)]
mod tests {
    use super::*;
    const PACKAGE: &str = "package_name/interface_name@1.0.0";
    const INTERFACE_ONLY: &str = "interface_name";
    const PACKAGE_WITHOUT_VERSION: &str = "package_name/interface_name";

    #[test]
    fn test_path_display() {
        for package in [PACKAGE, PACKAGE_WITHOUT_VERSION] {
            let path = InterfacePath::from_str(package).unwrap();
            assert_eq!(package, format!("{path}"));
            let foreign_path = path.clone().into_foreign().unwrap();
            assert_eq!(package, format!("{foreign_path}"));
        }

        let interface_only = InterfacePath::from_str(INTERFACE_ONLY).unwrap();
        assert!(interface_only.clone().into_foreign().is_none());
    }

    #[test]
    fn test_interface_path_roundtrip() {
        let path = InterfacePath::from_str(PACKAGE).unwrap();
        // Convert to ForeignInterfacePath and back
        assert_eq!(path, path.clone().into_foreign().unwrap().into());

        let interface_only = InterfacePath::from_str(INTERFACE_ONLY).unwrap();
        assert_eq!(None, interface_only.clone().into_foreign());

        // Parse the string representation back into InterfacePath
        assert_eq!(
            path,
            InterfacePath::new(
                path.package_name().map(String::from),
                path.interface_name().to_string(),
                path.version().cloned(),
            )
        );
    }

    #[test]
    fn test_foreign_interface_path_roundtrip() {
        for package in [PACKAGE, PACKAGE_WITHOUT_VERSION] {
            let path = InterfacePath::from_str(package).unwrap();
            let foreign_path: ForeignInterfacePath = path.clone().into_foreign().unwrap();

            assert_eq!(
                foreign_path,
                ForeignInterfacePath::new(
                    path.package_name().unwrap().to_string(),
                    path.interface_name().to_string(),
                    path.version().cloned()
                )
            );
        }
    }

    #[test]
    fn test_foreign_interface_path() {
        let path = InterfacePath::from_str(PACKAGE).unwrap();
        let foreign_path: ForeignInterfacePath = path.clone().into_foreign().unwrap();
        assert_eq!(foreign_path.package_name(), "package_name");
        assert_eq!(foreign_path.interface_name(), "interface_name");
        assert_eq!(
            foreign_path.version(),
            Some(&Version::parse("1.0.0").unwrap())
        );

        let fp_string = foreign_path.to_string();
        assert_eq!(PACKAGE, fp_string);
        assert_eq!(PACKAGE, InterfacePath::from(foreign_path).to_string());
        assert_eq!(fp_string, path.to_string());
    }

    #[test]
    fn test_interface_path_parsing() {
        let path = InterfacePath::from_str(PACKAGE).unwrap();
        assert_eq!(path.package_name(), Some("package_name"));
        assert_eq!(path.interface_name(), "interface_name");
        assert_eq!(path.version(), Some(&Version::parse("1.0.0").unwrap()));
        assert_eq!(path.to_string(), PACKAGE);

        let path = InterfacePath::from_str("interface_name").unwrap();
        assert_eq!(path.package_name(), None);
        assert_eq!(path.interface_name(), "interface_name");
        assert_eq!(path.version(), None);
        assert_eq!(path.to_string(), "interface_name");

        let path = InterfacePath::from_str("package_name/interface_name").unwrap();
        assert_eq!(path.package_name(), Some("package_name"));
        assert_eq!(path.interface_name(), "interface_name");
        assert_eq!(path.version(), None);
        assert_eq!(path.to_string(), "package_name/interface_name");

        let path_err = InterfacePath::from_str("package_name/interface_name/").unwrap_err();
        assert!(matches!(path_err, InterfacePathParseError::FormatError));

        let path_err = InterfacePath::from_str("package_name/interface_name@").unwrap_err();
        assert!(matches!(
            path_err,
            InterfacePathParseError::VersionParseError { .. }
        ));
    }
}
