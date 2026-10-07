//! Windows settles on a 60 ms connection interval (about 16 reports a second) unless asked
//! for a short one while a Joy-Con is linked; the pointer then moves in visible steps.

pub fn interval_ms(units: u16) -> f64 {
    f64::from(units) * 1.25
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
}
