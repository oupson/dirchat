use std::{
    net::{ToSocketAddrs, UdpSocket},
    sync::Arc,
};

use anyhow::Context;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleRate,
};

use ringbuf::{
    traits::{Consumer, Producer, Split},
    HeapRb,
};

fn main() -> anyhow::Result<()> {
    let host = cpal::default_host();

    let input_device = host
        .default_input_device()
        .context("no input device available")?;

    let output_device = host
        .default_output_device()
        .context("no output device available")?;

    let bind = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:3000".to_owned());
    let udp_socket = UdpSocket::bind(bind)?;

    let send_addr = std::env::args()
        .skip(1)
        .next()
        .unwrap()
        .to_socket_addrs()?
        .next()
        .unwrap();

    let ring = HeapRb::<u8>::new(48_000 * 2);
    let (mut producer, mut consumer) = ring.split();

    let udp_socket = Arc::new(udp_socket);

    let input_stream = {
        let udp_socket = udp_socket.clone();
        input_device
            .build_input_stream(
                &cpal::StreamConfig {
                    channels: 1,
                    sample_rate: SampleRate(48_000),
                    buffer_size: cpal::BufferSize::Fixed(2048),
                },
                move |d: &[u8], _| {
                    udp_socket.send_to(d, &send_addr).unwrap();
                },
                err_fn,
                None,
            )
            .context("failed to create input stream")?
    };
    let output_stream = output_device
        .build_output_stream(
            &cpal::StreamConfig {
                channels: 1,
                sample_rate: SampleRate(48_000),
                buffer_size: cpal::BufferSize::Fixed(2048),
            },
            move |data: &mut [u8], _: &cpal::OutputCallbackInfo| {
                for sample in data {
                    *sample = match consumer.try_pop() {
                        Some(s) => s,
                        None => 0,
                    };
                }
            },
            err_fn,
            None,
        )
        .context("failed to create input stream")?;

    input_stream.play()?;
    output_stream.play()?;

    let mut buf = [0u8; 48_000];
    loop {
        let size = udp_socket.recv(&mut buf)?;
        producer.push_slice(&buf[0..size]);
    }
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}
