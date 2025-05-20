use derivative::Derivative;
use semver::Version;
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Derivative, Debug)]
#[derivative(Default(bound = ""))]
pub struct VersionMap<T> {
    versions: BTreeMap<Version, T>,
    alternates: HashMap<Version, BTreeSet<Version>>,
}

impl<T> VersionMap<T> {
    pub fn new() -> Self {
        Self {
            versions: BTreeMap::new(),
            alternates: HashMap::new(),
        }
    }

    pub fn try_insert(&mut self, version: Version, value: T) -> Result<(), (Version, T)> {
        if self.versions.contains_key(&version) {
            return Err((version, value));
        }

        if let Some(alternate) = version_alternate(&version) {
            self.alternates
                .entry(alternate)
                .or_default()
                .insert(version.clone());
        }

        self.versions.insert(version, value);

        Ok(())
    }

    pub fn insert(&mut self, version: Version, value: T) -> Option<T> {
        if let Some(alternate) = version_alternate(&version) {
            self.alternates
                .entry(alternate)
                .or_default()
                .insert(version.clone());
        }

        self.versions.insert(version, value)
    }

    pub fn get(&self, version: &Version) -> Option<&T> {
        if version.build.is_empty() {
            let maybe_value = version_alternate(version)
                .as_ref()
                .and_then(|alternate| self.alternates.get(alternate))
                .and_then(|version_set| version_set.last())
                .and_then(|version| self.versions.get(version));

            if maybe_value.is_some() {
                return maybe_value;
            }
        }

        self.get_exact(version)
    }

    pub fn get_or_latest(&self, version: Option<&Version>) -> Option<&T> {
        if let Some(version) = version {
            self.get(version)
        } else {
            self.get_latest().map(|(_, value)| value)
        }
    }

    pub fn get_latest(&self) -> Option<(&Version, &T)> {
        self.versions.last_key_value()
    }

    pub fn get_exact(&self, version: &Version) -> Option<&T> {
        self.versions.get(version)
    }

    pub fn remove(&mut self, version: &Version) -> Option<T> {
        if let Some(alternate) = version_alternate(version) {
            if let Some(set) = self.alternates.get_mut(&alternate) {
                set.remove(version);
                if set.is_empty() {
                    self.alternates.remove(&alternate);
                }
            }
        }

        self.versions.remove(version)
    }
}

fn version_alternate(version: &Version) -> Option<Version> {
    if !version.pre.is_empty() {
        None
    } else if version.major > 0 {
        Some(Version::new(version.major, 0, 0))
    } else if version.minor > 0 {
        Some(Version::new(0, version.minor, 0))
    } else {
        Some(Version::new(0, 0, version.patch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;

    #[test]
    fn test_version_map() {
        let mut map = VersionMap::new();

        let version0 = Version::new(0, 4, 2);
        let version1 = Version::new(1, 0, 0);
        let version2 = Version::new(1, 0, 1);
        let version3 = Version::new(2, 0, 0);

        map.try_insert(version0.clone(), "value0").unwrap();
        map.try_insert(version1.clone(), "value1").unwrap();
        map.try_insert(version2.clone(), "value2").unwrap();
        map.try_insert(version3.clone(), "value3").unwrap();

        assert_eq!(map.get(&version0), Some(&"value0"));
        assert_eq!(map.get(&version1), Some(&"value2"));
        assert_eq!(map.get(&version2), Some(&"value2"));
        assert_eq!(map.get(&version3), Some(&"value3"));

        assert_eq!(map.get(&Version::new(0, 1, 0)), None);
        assert_eq!(map.get(&Version::new(0, 4, 1)), Some(&"value0"));
        assert_eq!(map.get(&Version::new(1, 1, 0)), Some(&"value2"));
        assert_eq!(map.get(&Version::new(2, 0, 4)), Some(&"value3"));
        assert_eq!(map.get(&Version::new(3, 0, 0)), None);

        assert_eq!(map.get_exact(&version1), Some(&"value1"));

        assert_eq!(map.get_or_latest(None), Some(&"value3"));
        assert_eq!(map.get_or_latest(Some(&version1)), Some(&"value2"));
    }
}
