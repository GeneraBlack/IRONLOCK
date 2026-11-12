use super::*;

#[test]
fn encodes_spec_compliant_ktx2_srgb_and_unorm_with_full_1x1_mip_chain() {
    let (w, h) = (96u32, 72u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            rgba[idx] = ((x * 255) / w) as u8;
            rgba[idx + 1] = ((y * 255) / h) as u8;
            rgba[idx + 2] = 180;
            rgba[idx + 3] = 255;
        }
    }

    let srgb_bytes = encode_rgba8_to_ktx2(w, h, &rgba, Ktx2ColorSpace::Srgb, MipFilter::Lanczos3)
        .expect("sRGB KTX2 encode should succeed");
    let info = inspect_ktx2_header(&srgb_bytes).expect("inspect_ktx2_header should succeed");
    assert_eq!(&srgb_bytes[0..12], &KTX2_MAGIC);
    assert_eq!(info.vk_format, VK_FORMAT_R8G8B8A8_SRGB);
    assert_eq!(info.width, 96);
    assert_eq!(info.height, 72);
    assert_eq!(info.level_count, 7); // 96x72 -> 48x36 -> 24x18 -> 12x9 -> 6x4 -> 3x2 -> 1x1
    assert_eq!(info.dfd_byte_length as usize, KTX2_RGBA8_DFD_BYTES);
    assert_eq!(
        info.levels.last().map(|l| (l.width, l.height)),
        Some((1, 1))
    );
    assert_eq!(info.levels.last().map(|l| l.byte_length), Some(4));

    // Parse with the exact `ktx2` crate used by Bevy 0.15's `bevy_image` loader!
    let reader = ktx2::Reader::new(&srgb_bytes).expect("ktx2::Reader must accept container");
    assert_eq!(reader.header().format, Some(ktx2::Format::R8G8B8A8_SRGB));
    assert_eq!(reader.header().level_count, 7);
    assert_eq!(reader.header().supercompression_scheme, None);

    let dfds: Vec<_> = reader.data_format_descriptors().collect();
    assert_eq!(dfds.len(), 1);
    assert_eq!(dfds[0].header, ktx2::DataFormatDescriptorHeader::BASIC);
    let basic = ktx2::BasicDataFormatDescriptor::parse(dfds[0].data).expect("Basic DFD must parse");
    assert_eq!(basic.color_model, Some(ktx2::ColorModel::RGBSDA));
    assert_eq!(basic.transfer_function, Some(ktx2::TransferFunction::SRGB));
    let samples: Vec<_> = basic.sample_information().collect();
    assert_eq!(samples.len(), 4);
    assert_eq!(samples[0].channel_type, 0);
    assert_eq!(samples[1].channel_type, 1);
    assert_eq!(samples[2].channel_type, 2);
    assert_eq!(samples[3].channel_type, 15);

    let levels: Vec<_> = reader.levels().collect();
    assert_eq!(levels.len(), 7);
    assert_eq!(levels[0], rgba.as_slice());
    assert_eq!(levels[6].len(), 4);

    // Also verify LinearUnorm (Normal / ORM maps) with Box filter
    let lin_bytes = encode_rgba8_to_ktx2(
        16,
        16,
        &vec![128u8; 16 * 16 * 4],
        Ktx2ColorSpace::LinearUnorm,
        MipFilter::Box,
    )
    .expect("LinearUnorm KTX2 encode should succeed");
    let lin_reader = ktx2::Reader::new(&lin_bytes).expect("LinearUnorm ktx2::Reader");
    assert_eq!(
        lin_reader.header().format,
        Some(ktx2::Format::R8G8B8A8_UNORM)
    );
    assert_eq!(lin_reader.header().level_count, 5); // 16 -> 8 -> 4 -> 2 -> 1
    assert_eq!(
        infer_texture_color_space("assets/textures/hull_normal.png"),
        Ktx2ColorSpace::LinearUnorm
    );
    assert_eq!(
        infer_texture_color_space("assets/textures/hull_orm.png"),
        Ktx2ColorSpace::LinearUnorm
    );
    assert_eq!(
        infer_texture_color_space("assets/ui/cameos/tank_icon.png"),
        Ktx2ColorSpace::Srgb
    );
}
