//! Windows settles on a 60 ms connection interval (about 16 reports a second) unless asked
//! for a short one while a Joy-Con is linked; the pointer then moves in visible steps.

pub fn interval_ms(units: u16) -> f64 {
    f64::from(units) * 1.25
}

/// Keeps a finished request only if its slot, made at connect, survived: a disconnect while
/// it was pending removed the slot, so the link is dropped (closing the request) instead.
#[cfg(any(windows, test))]
pub fn settle<K: Eq + std::hash::Hash, V>(
    slots: &mut std::collections::HashMap<K, Option<V>>,
    id: &K,
    link: V,
) -> bool {
    match slots.get_mut(id) {
        Some(slot) => {
            *slot = Some(link);
            true
        }
        None => false,
    }
}

#[cfg(windows)]
pub use imp::FastLink;

#[cfg(windows)]
mod imp {
    use btleplug::api::BDAddr;
    use btleplug::platform::PeripheralId;
    use windows::Devices::Bluetooth::{
        BluetoothLEDevice, BluetoothLEPreferredConnectionParameters,
        BluetoothLEPreferredConnectionParametersRequest,
        BluetoothLEPreferredConnectionParametersRequestStatus as RequestStatus,
    };

    /// Windows honors this request only while it stays open, so it lives as long
    /// as the link.
    pub struct FastLink {
        device: BluetoothLEDevice,
        request: BluetoothLEPreferredConnectionParametersRequest,
    }

    impl FastLink {
        pub async fn request(id: &PeripheralId) -> windows::core::Result<Self> {
            let address = u64::from(BDAddr::from(id.clone()));
            let device = BluetoothLEDevice::FromBluetoothAddressAsync(address)?.await?;
            let preferred = BluetoothLEPreferredConnectionParameters::ThroughputOptimized()?;
            let request = device.RequestPreferredConnectionParameters(&preferred)?;
            Ok(Self { device, request })
        }

        pub fn status(&self) -> &'static str {
            match self.request.Status() {
                Ok(RequestStatus::Success) => "accepted",
                Ok(RequestStatus::DeviceNotAvailable) => "device not available",
                Ok(RequestStatus::AccessDenied) => "access denied",
                Ok(_) => "unspecified",
                Err(_) => "unknown",
            }
        }

        pub fn interval_ms(&self) -> Option<f64> {
            let parameters = self.device.GetConnectionParameters().ok()?;
            parameters.ConnectionInterval().ok().map(super::interval_ms)
        }
    }

    impl Drop for FastLink {
        fn drop(&mut self) {
            let _ = self.request.Close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_are_in_bluetooth_units() {
        assert_eq!(interval_ms(6), 7.5);
        assert_eq!(interval_ms(12), 15.0);
        assert_eq!(interval_ms(48), 60.0);
    }

    #[test]
    fn a_request_that_outlives_its_link_is_dropped() {
        use std::collections::HashMap;
        use std::rc::Rc;
        let link = Rc::new(());
        let mut slots = HashMap::from([("joy-con", None)]);
        assert!(settle(&mut slots, &"joy-con", Rc::clone(&link)));
        assert_eq!(Rc::strong_count(&link), 2, "kept while linked");

        slots.remove("joy-con");
        assert!(!settle(&mut slots, &"joy-con", Rc::clone(&link)));
        assert!(slots.is_empty());
        assert_eq!(Rc::strong_count(&link), 1, "dropped, so the request closes");
    }
}
