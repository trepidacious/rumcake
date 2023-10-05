use embassy_nrf::bind_interrupts;
use embassy_nrf::interrupt::{InterruptExt, Priority};
use embassy_nrf::nvmc::Nvmc;
use embassy_nrf::peripherals::SAADC;
use embassy_nrf::saadc::{ChannelConfig, Input, Saadc, VddhDiv5Input};
use embassy_nrf::usb::Driver;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embedded_storage::nor_flash::NorFlash;
use lazy_static::lazy_static;
use static_cell::StaticCell;

#[cfg(feature = "nrf52840")]
pub const SYSCLK: u32 = 48_000_000;

pub fn initialize_rcc() {
    let mut conf = embassy_nrf::config::Config::default();
    conf.time_interrupt_priority = Priority::P2;
    embassy_nrf::init(conf);
}

#[macro_export]
macro_rules! input_pin {
    ($p:ident) => {
        unsafe {
            $crate::embassy_nrf::gpio::Input::new(
                $crate::embassy_nrf::gpio::Pin::degrade(
                    $crate::embassy_nrf::peripherals::$p::steal(),
                ),
                $crate::embassy_nrf::gpio::Pull::Up,
            )
        }
    };
}

#[macro_export]
macro_rules! output_pin {
    ($p:ident) => {
        unsafe {
            $crate::embassy_nrf::gpio::Output::new(
                $crate::embassy_nrf::gpio::Pin::degrade(
                    $crate::embassy_nrf::peripherals::$p::steal(),
                ),
                $crate::embassy_nrf::gpio::Level::High,
                $crate::embassy_nrf::gpio::OutputDrive::Standard,
            )
        }
    };
}

#[cfg(feature = "nrf-ble")]
lazy_static! {
    static ref VBUS_DETECT: embassy_nrf::usb::vbus_detect::SoftwareVbusDetect =
        embassy_nrf::usb::vbus_detect::SoftwareVbusDetect::new(false, false);
}

#[cfg(feature = "usb")]
pub fn setup_usb_driver<K: crate::usb::USBKeyboard + 'static>() -> embassy_usb::Builder<
    'static,
    Driver<'static, embassy_nrf::peripherals::USBD, impl embassy_nrf::usb::vbus_detect::VbusDetect>,
> {
    unsafe {
        #[cfg(feature = "nrf52840")]
        bind_interrupts!(
            struct Irqs {
                USBD => embassy_nrf::usb::InterruptHandler<embassy_nrf::peripherals::USBD>;
                POWER_CLOCK => embassy_nrf::usb::vbus_detect::InterruptHandler;
            }
        );

        embassy_nrf::interrupt::USBD.set_priority(embassy_nrf::interrupt::Priority::P2);
        embassy_nrf::interrupt::POWER_CLOCK.set_priority(embassy_nrf::interrupt::Priority::P2);

        let mut config = embassy_usb::Config::new(K::USB_VID, K::USB_PID);
        config.manufacturer.replace(K::MANUFACTURER);
        config.product.replace(K::PRODUCT);
        config.serial_number.replace(K::SERIAL_NUMBER);
        config.max_power = 100;

        let usb_driver = Driver::new(
            embassy_nrf::peripherals::USBD::steal(),
            Irqs,
            #[cfg(feature = "nrf-ble")]
            &*VBUS_DETECT,
            #[cfg(not(feature = "nrf-ble"))]
            embassy_nrf::usb::vbus_detect::HardwareVbusDetect::new(Irqs),
        );

        static DEVICE_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        let device_descriptor = DEVICE_DESCRIPTOR.init([0; 256]);
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        let config_descriptor = CONFIG_DESCRIPTOR.init([0; 256]);
        static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        let bos_descriptor = BOS_DESCRIPTOR.init([0; 256]);
        static CONTROL_BUF: StaticCell<[u8; 128]> = StaticCell::new();
        let control_buf = CONTROL_BUF.init([0; 128]);

        embassy_usb::Builder::new(
            usb_driver,
            config,
            device_descriptor,
            config_descriptor,
            bos_descriptor,
            control_buf,
        )
    }
}

pub fn setup_flash() -> &'static mut Mutex<ThreadModeRawMutex, impl NorFlash> {
    unsafe {
        static FLASH_PERIPHERAL: StaticCell<Mutex<ThreadModeRawMutex, Nvmc>> = StaticCell::new();

        FLASH_PERIPHERAL.init(Mutex::new(Nvmc::new(
            embassy_nrf::peripherals::NVMC::steal(),
        )))
    }
}

pub fn setup_adc() -> Saadc<'static, 1> {
    unsafe {
        bind_interrupts! {
            struct Irqs {
                SAADC => embassy_nrf::saadc::InterruptHandler;
            }
        }
        embassy_nrf::interrupt::SAADC.set_priority(embassy_nrf::interrupt::Priority::P2);
        let vddh = VddhDiv5Input;
        let channel = ChannelConfig::single_ended(vddh.degrade_saadc());
        Saadc::new(SAADC::steal(), Irqs, Default::default(), [channel])
    }
}

pub fn adc_sample_to_pct(sample: &i16) -> u8 {
    let mv = sample * 5;

    if mv >= 4200 {
        100
    } else if mv <= 3450 {
        0
    } else {
        (mv * 2 / 15 - 459) as u8
    }
}

#[macro_export]
macro_rules! setup_i2c {
    ($interrupt:ident, $i2c:ident, $sda:ident, $scl:ident) => {
        fn setup_i2c() -> impl $crate::embedded_hal_async::i2c::I2c<Error = impl core::fmt::Debug> {
            use $crate::embassy_nrf::interrupt::InterruptExt;
            unsafe {
                $crate::embassy_nrf::bind_interrupts! {
                    struct Irqs {
                        $interrupt => $crate::embassy_nrf::twim::InterruptHandler<$crate::embassy_nrf::peripherals::$i2c>
                    }
                };
                $crate::embassy_nrf::interrupt::$interrupt.set_priority($crate::embassy_nrf::interrupt::Priority::P2);
                let i2c = $crate::embassy_nrf::peripherals::$i2c::steal();
                let sda = $crate::embassy_nrf::peripherals::$sda::steal();
                let scl = $crate::embassy_nrf::peripherals::$scl::steal();
                $crate::embassy_nrf::twim::Twim::new(i2c, Irqs, sda, scl, Default::default())
            }
        }
    };
}

#[cfg(feature = "nrf-ble")]
pub fn setup_softdevice<K: crate::keyboard::Keyboard>() -> &'static mut nrf_softdevice::Softdevice
where
    [(); K::PRODUCT.len()]:,
{
    let config = nrf_softdevice::Config {
        clock: Some(nrf_softdevice::raw::nrf_clock_lf_cfg_t {
            source: nrf_softdevice::raw::NRF_CLOCK_LF_SRC_XTAL as u8,
            rc_ctiv: 0,
            rc_temp_ctiv: 0,
            accuracy: nrf_softdevice::raw::NRF_CLOCK_LF_ACCURACY_20_PPM as u8,
        }),
        gatts_attr_tab_size: Some(nrf_softdevice::raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: 32768,
        }),
        conn_gap: Some(nrf_softdevice::raw::ble_gap_conn_cfg_t {
            conn_count: 6,
            event_length: 24,
        }),
        conn_gatt: Some(nrf_softdevice::raw::ble_gatt_conn_cfg_t { att_mtu: 256 }),
        gap_role_count: Some(nrf_softdevice::raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 3,
            central_role_count: 3,
            central_sec_count: 0,
            _bitfield_1: nrf_softdevice::raw::ble_gap_cfg_role_count_t::new_bitfield_1(0),
        }),
        gap_device_name: Some(nrf_softdevice::raw::ble_gap_cfg_device_name_t {
            p_value: K::PRODUCT.as_ptr() as _,
            current_len: K::PRODUCT.len() as u16,
            max_len: K::PRODUCT.len() as u16,
            write_perm: unsafe { core::mem::zeroed() },
            _bitfield_1: nrf_softdevice::raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                nrf_softdevice::raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    };

    nrf_softdevice::Softdevice::enable(&config)
}

#[cfg(feature = "nrf-ble")]
#[rumcake_macros::task]
pub async fn softdevice_task(sd: &'static nrf_softdevice::Softdevice) {
    sd.run_with_callback(|e| match e {
        nrf_softdevice::SocEvent::PowerUsbPowerReady => {
            VBUS_DETECT.ready();
        }
        nrf_softdevice::SocEvent::PowerUsbDetected => {
            VBUS_DETECT.detected(true);
        }
        nrf_softdevice::SocEvent::PowerUsbRemoved => {
            VBUS_DETECT.detected(false);
        }
        _ => {}
    })
    .await
}