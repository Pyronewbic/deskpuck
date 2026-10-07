//! Windows asks for a short connection interval while a Joy-Con is linked.
//! Left alone, Windows settles on 60 ms a second after connecting, about 16
//! reports a second; the pointer then moves in visible steps.

/// A connection interval in milliseconds, from the 1.25 ms units Bluetooth uses.
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

    /// An open request for Windows' throughput-optimized connection
    /// parameters. Windows honors it only while it stays open, so it lives as
    /// long as the link; dropping it hands the choice back to Windows.
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

        /// The interval Windows is using now, which can lag the request.
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
