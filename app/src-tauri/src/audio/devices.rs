//! Snapshot-backed choices. Menu positions never identify devices in a later enumeration.
use cpal::traits::{DeviceTrait, HostTrait};
use eyre::{bail, eyre, Result};

#[derive(Clone)]
pub struct DeviceKey {
    // Retain native endpoint identity, not the mutable friendly name. ALSA Device objects cache
    // open PCM handles during discovery; keep only its backend name there so unused devices close.
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    device: cpal::Device,
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    name: String,
}

impl DeviceKey {
    fn new(device: &cpal::Device, name: &str) -> Self {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            let _ = name;
            Self {
                device: device.clone(),
            }
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = device;
            Self { name: name.into() }
        }
    }
}

impl PartialEq for DeviceKey {
    fn eq(&self, other: &Self) -> bool {
        // CPAL 0.15 exposes endpoint equality on the native Windows/macOS backends, not on its
        // dynamic wrapper. Leave a fallback for builds that enable another host backend.
        #[cfg(target_os = "windows")]
        #[allow(unreachable_patterns)]
        match (self.device.as_inner(), other.device.as_inner()) {
            (cpal::platform::DeviceInner::Wasapi(a), cpal::platform::DeviceInner::Wasapi(b)) => {
                a == b
            }
            _ => false,
        }
        #[cfg(target_os = "macos")]
        #[allow(unreachable_patterns)]
        match (self.device.as_inner(), other.device.as_inner()) {
            (
                cpal::platform::DeviceInner::CoreAudio(a),
                cpal::platform::DeviceInner::CoreAudio(b),
            ) => a == b,
            _ => false,
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            self.name == other.name
        }
    }
}

#[derive(Clone)]
pub enum Selection<K = DeviceKey> {
    /// Deliberately follows the OS default when a new stream starts.
    Default,
    Endpoint {
        name: String,
        key: K,
    },
}

impl<K: PartialEq> Selection<K> {
    #[cfg(test)]
    fn same_target(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Default, Self::Default) => true,
            (Self::Endpoint { key: a, .. }, Self::Endpoint { key: b, .. }) => a == b,
            _ => false,
        }
    }
}

pub struct Entry<K = DeviceKey> {
    pub label: String,
    pub selection: Selection<K>,
}

pub struct DeviceList<K = DeviceKey> {
    pub entries: Vec<Entry<K>>,
    /// Entry to select on first load: the default, or the first device without one.
    #[cfg_attr(not(test), allow(dead_code))]
    initial_index: i32,
}

impl<K> Default for DeviceList<K> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            initial_index: -1,
        }
    }
}

impl<K: Clone + PartialEq> DeviceList<K> {
    fn new(default_name: Option<String>, named: Vec<(String, K)>) -> Self {
        if default_name.is_none() && named.is_empty() {
            return Self::default();
        }
        let initial_index = i32::from(default_name.is_none());
        let mut entries = vec![Entry {
            label: format!(
                "System default ({})",
                menu_name(default_name.as_deref().unwrap_or("none"))
            ),
            selection: Selection::Default,
        }];
        let labels: Vec<_> = named.iter().map(|(name, _)| menu_name(name)).collect();
        for (i, (name, key)) in named.iter().enumerate() {
            let duplicate = labels.iter().filter(|label| **label == labels[i]).count() > 1;
            let mut label = if duplicate {
                format!("{} [device {}]", labels[i], i + 1)
            } else {
                labels[i].clone()
            };
            while entries.iter().any(|entry| entry.label == label) {
                label.push_str(" •");
            }
            entries.push(Entry {
                label,
                selection: Selection::Endpoint {
                    name: name.clone(),
                    key: key.clone(),
                },
            });
        }
        Self {
            entries,
            initial_index,
        }
    }

    #[cfg(test)]
    pub fn selection(&self, index: i32) -> Result<Selection<K>> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.entries.get(i))
            .map(|e| e.selection.clone())
            .ok_or_else(|| eyre!("Choose an available audio device; refresh the list if needed"))
    }

    /// A disappeared explicit endpoint stays unselected, even if a different default exists.
    #[cfg(test)]
    pub fn refreshed_index(&self, previous: Option<&Selection<K>>, first_load: bool) -> i32 {
        if first_load {
            return self.initial_index;
        }
        let Some(previous) = previous else {
            return -1;
        };
        let mut matches = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.selection.same_target(previous));
        match (matches.next(), matches.next()) {
            (Some((i, _)), None) => i as i32,
            _ => -1,
        }
    }

    #[allow(dead_code)] // Exact-name interface used by the shared physical-device tester.
    pub fn named(&self, name: &str) -> Result<Selection<K>> {
        let mut matches = self
            .entries
            .iter()
            .filter(|e| matches!(&e.selection, Selection::Endpoint { name: n, .. } if n == name));
        match (matches.next(), matches.next()) {
            (Some(entry), None) => Ok(entry.selection.clone()),
            (None, _) => {
                bail!("Device {name:?} is unavailable; refresh the list");
            }
            _ => {
                bail!(
                    "More than one device is named {name:?}; choose an explicit entry in the GUI"
                );
            }
        }
    }

    #[allow(dead_code)] // Raw backend names printed by the shared physical-device tester.
    pub fn names(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter_map(|e| match &e.selection {
                Selection::Endpoint { name, .. } => Some(name.as_str()),
                Selection::Default => None,
            })
            .collect()
    }
}

fn menu_name(name: &str) -> String {
    // FLTK interprets menu separators, submenus, shortcuts and escape prefixes. These display
    // replacements never participate in identity matching or the CLI's exact raw-name lookup.
    name.chars()
        .map(|c| match c {
            '|' => '¦',
            '/' => '∕',
            '\\' => '⧵',
            '&' => '＆',
            '\t' | '\n' | '\r' => ' ',
            _ => c,
        })
        .collect()
}

fn direction_devices(host: &cpal::Host, input: bool) -> Result<Vec<cpal::Device>> {
    // Preserve macOS name-only discovery: direction probing can request microphone access.
    #[cfg(target_os = "macos")]
    {
        let _ = input;
        Ok(host.devices()?.collect())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(if input {
            host.input_devices()?.collect()
        } else {
            host.output_devices()?.collect()
        })
    }
}

pub fn discover(input: bool) -> Result<DeviceList> {
    let host = cpal::default_host();
    let default = if input {
        host.default_input_device()
    } else {
        host.default_output_device()
    };
    let default_name = default.map(|d| d.name().unwrap_or_else(|_| "unavailable name".into()));
    let mut named = Vec::new();
    for device in direction_devices(&host, input)? {
        // Never synthesize an identity from the ordinal if a backend cannot name the endpoint.
        let name = device.name()?;
        named.push((name.clone(), DeviceKey::new(&device, &name)));
    }
    Ok(DeviceList::new(default_name, named))
}

fn unique_index<K: PartialEq>(target: &K, keys: impl Iterator<Item = K>) -> Result<usize> {
    let mut result = None;
    for (i, key) in keys.enumerate() {
        if key == *target {
            if result.is_some() {
                bail!("Device identity is ambiguous; refresh and choose another device");
            }
            result = Some(i);
        }
    }
    result.ok_or_else(|| {
        eyre!("Selected device is no longer available; reconnect it and refresh devices")
    })
}

pub fn resolve(input: bool, selected: &Selection) -> Result<cpal::Device> {
    let host = cpal::default_host();
    match selected {
        Selection::Default => (if input {
            host.default_input_device()
        } else {
            host.default_output_device()
        })
        .ok_or_else(|| eyre!("No default audio device is available")),
        Selection::Endpoint { name, key } => {
            let current = direction_devices(&host, input)?;
            let keys = current
                .iter()
                .map(|d| d.name().map(|n| DeviceKey::new(d, &n)))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let index = unique_index(key, keys.into_iter()).map_err(|e| {
                eyre!(
                    "{} {name:?}: {e}",
                    if input { "Microphone" } else { "Speakers" }
                )
            })?;
            Ok(current
                .into_iter()
                .nth(index)
                .expect("matched current endpoint"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn list(names: &[(&str, u32)]) -> DeviceList<u32> {
        DeviceList::new(
            Some("OS default".into()),
            names.iter().map(|(n, k)| (n.to_string(), *k)).collect(),
        )
    }
    #[test]
    fn reorder_and_rename_preserve_identity_not_menu_index_or_label() {
        let old = list(&[("USB", 7), ("Internal", 9)]).selection(1).unwrap();
        let new = list(&[("Internal", 9), ("Renamed USB", 7)]);
        assert_eq!(new.refreshed_index(Some(&old), false), 2);
        assert_eq!(unique_index(&7, [9, 7].into_iter()).unwrap(), 1);
    }
    #[test]
    fn removal_does_not_fall_back_to_a_replacement_or_default() {
        let old = list(&[("USB", 7)]).selection(1).unwrap();
        let new = list(&[("USB", 8), ("Internal", 9)]);
        assert_eq!(new.refreshed_index(Some(&old), false), -1);
        assert!(unique_index(&7, [8, 9].into_iter()).is_err());
        assert!(new.selection(-1).is_err());
        assert_eq!(new.refreshed_index(None, false), -1);
    }
    #[test]
    fn duplicate_names_remain_distinct_and_named_cli_selection_is_rejected() {
        let devices = list(&[("USB", 7), ("USB", 8)]);
        assert!(devices.named("USB").is_err());
        assert_ne!(devices.entries[1].label, devices.entries[2].label);
        assert!(!devices
            .selection(1)
            .unwrap()
            .same_target(&devices.selection(2).unwrap()));
        assert!(unique_index(&7, [7, 7].into_iter()).is_err());
        let ambiguous = list(&[("USB", 7), ("USB alias", 7)]);
        assert_eq!(
            ambiguous.refreshed_index(Some(&devices.selection(1).unwrap()), false),
            -1
        );
    }
    #[test]
    fn default_is_dynamic_but_never_implicit_after_an_explicit_choice() {
        let devices = list(&[("USB", 7)]);
        assert_eq!(devices.refreshed_index(None, true), 0);
        assert_eq!(devices.refreshed_index(Some(&Selection::Default), false), 0);
        let without_default = DeviceList::new(None, vec![("USB".into(), 7)]);
        assert_eq!(without_default.refreshed_index(None, true), 1);
        assert_eq!(DeviceList::<u32>::default().refreshed_index(None, true), -1);
    }
    #[test]
    fn menu_metacharacters_never_change_raw_name_matching() {
        let raw = "A/B|C&D\\E\tF";
        let devices = list(&[(raw, 7), ("A∕B¦C＆D⧵E F", 8)]);
        assert_ne!(devices.entries[1].label, devices.entries[2].label);
        assert!(devices.named(raw).is_ok());
        assert_eq!(devices.names(), [raw, "A∕B¦C＆D⧵E F"]);
        assert!(!devices
            .named(raw)
            .unwrap()
            .same_target(&devices.named("A∕B¦C＆D⧵E F").unwrap()));
    }
}
