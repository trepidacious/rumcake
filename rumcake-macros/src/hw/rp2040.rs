use proc_macro2::{Ident, TokenStream};
use proc_macro_error::{abort, OptionExt};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::Token;

pub fn input_pin(ident: Ident) -> TokenStream {
    quote! {
        unsafe {
            ::rumcake::hw::mcu::embassy_rp::gpio::Input::new(
                ::rumcake::hw::mcu::embassy_rp::gpio::Pin::degrade(
                    ::rumcake::hw::mcu::embassy_rp::peripherals::#ident::steal(),
                ),
                ::rumcake::hw::mcu::embassy_rp::gpio::Pull::Up,
            )
        }
    }
}

pub fn output_pin(ident: Ident) -> TokenStream {
    quote! {
        unsafe {
            ::rumcake::hw::mcu::embassy_rp::gpio::Output::new(
                ::rumcake::hw::mcu::embassy_rp::gpio::Pin::degrade(
                    ::rumcake::hw::mcu::embassy_rp::peripherals::#ident::steal(),
                ),
                ::rumcake::hw::mcu::embassy_rp::gpio::Level::High,
                ::rumcake::hw::mcu::embassy_rp::gpio::Speed::Low,
            )
        }
    }
}

fn setup_i2c_inner(args: Punctuated<Ident, Token![,]>) -> TokenStream {
    let mut args = args.iter();

    let interrupt = args
        .next()
        .expect_or_abort("Missing interrupt argument.");
    let i2c = args
        .next()
        .expect_or_abort("Missing I2C peripheral argument.");
    let scl = args
        .next()
        .expect_or_abort("Missing SCL peripheral argument.");
    let sda = args
        .next()
        .expect_or_abort("Missing SDA peripheral argument.");

    if let Some(literal) = args.next() {
        abort!(literal.span(), "Unexpected extra arguments.")
    }

    quote! {
        unsafe {
            ::rumcake::hw::mcu::embassy_rp::bind_interrupts! {
                struct Irqs {
                    #interrupt => ::rumcake::hw::mcu::embassy_rp::i2c::InterruptHandler<::rumcake::hw::mcu::embassy_rp::peripherals::#i2c>;
                }
            };
            let i2c = ::rumcake::hw::mcu::embassy_rp::peripherals::#i2c::steal();
            let scl = ::rumcake::hw::mcu::embassy_rp::peripherals::#scl::steal();
            let sda = ::rumcake::hw::mcu::embassy_rp::peripherals::#sda::steal();
            let mut i2c_config = ::rumcake::hw::mcu::embassy_rp::i2c::Config::default();
            // TODO safer to default to 100kHz?
            i2c_config.frequency = 400_000;
        
            ::rumcake::hw::mcu::embassy_rp::i2c::I2c::new_async(i2c, scl, sda, Irqs, i2c_config)
        }
    }
}

pub fn setup_i2c(args: Punctuated<Ident, Token![,]>) -> TokenStream {
    let inner = setup_i2c_inner(args);
    quote! {
        fn setup_i2c() -> impl ::rumcake::embedded_hal_async::i2c::I2c<Error = impl core::fmt::Debug> {
            #inner
        }
    }
}
