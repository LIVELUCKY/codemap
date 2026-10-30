use super::*;

#[test]
fn half_precision_floats_are_read_exactly() {
    for (bits, value) in [
        (0x0000, 0.0),
        (0x3c00, 1.0),
        (0xbc00, -1.0),
        (0x3800, 0.5),
        (0x4000, 2.0),
        (0x3555, 0.333_251_95),
        (0x0001, 5.960_464_5e-8),
        (0x7bff, 65504.0),
    ] {
        assert_eq!(half_to_f32(bits), value, "{bits:#06x}");
    }
    assert!(half_to_f32(0x7c00).is_infinite());
    assert!(half_to_f32(0x7e00).is_nan());
}

#[test]
fn text_is_split_like_berts_tokenizer() {
    assert_eq!(
        words(&normalize("class BookRoute(val id: String)\n\tfun x() {}")),
        [
            "class",
            "bookroute",
            "(",
            "val",
            "id",
            ":",
            "string",
            ")",
            "fun",
            "x",
            "(",
            ")",
            "{",
            "}"
        ]
    );
    assert_eq!(words(&normalize("cafe\u{301} 中文")), ["cafe", "中", "文"]);
}

#[test]
fn a_word_becomes_its_longest_pieces_or_is_unknown_as_a_whole() {
    let model = model().expect("the bundled model loads");
    let ids = model.tokens("function");
    assert!(!ids.is_empty() && !ids.contains(&model.unknown));
    let long = "x".repeat(101);
    assert_eq!(model.tokens(&long), [model.unknown]);
    assert!(model.tokens("").is_empty());
}

#[test]
fn a_vector_has_length_one_and_survives_storage() {
    let vector = embed("database access object for orders").unwrap();
    assert_eq!(vector.len(), 256);
    let length: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    assert!((length - 1.0).abs() < 1e-4, "{length}");
    let stored = to_bytes(&vector);
    assert_eq!(stored.len(), 512);
    assert!((similarity(&vector, &stored) - 1.0).abs() < 1e-3);
    assert_eq!(
        similarity(&vector, &stored[..10]),
        0.0,
        "another model's vector"
    );
}

#[test]
fn a_description_is_closer_to_the_code_it_describes() {
    let query = embed("database access object for orders").unwrap();
    let dao = to_bytes(&embed("OrderDao.kt\ninterface OrderDao { fun insert(order: Order); fun findById(id: Long): Order? }").unwrap());
    let other = to_bytes(
        &embed("Theme.kt\nval primaryColor = Color(0xFF336699)\nval spacing = 8.dp").unwrap(),
    );
    assert!(similarity(&query, &dao) > similarity(&query, &other));
}

/// The first two dimensions of known texts, pinned so that a change to the tokenizer or to the table
/// reader shows: code, accented Latin letters, and the tokens that are matched as written.
#[test]
fn known_texts_keep_their_vectors() {
    for (text, first, second) in [
        ("fun findById(id: Long): Order?", PINNED[0].0, PINNED[0].1),
        ("Grüße aus München, señor café", PINNED[1].0, PINNED[1].1),
        ("[UNK] literal [PAD]", PINNED[2].0, PINNED[2].1),
    ] {
        let vector = embed(text).unwrap();
        assert!(
            (vector[0] - first).abs() < 1e-5,
            "{text}: {} {}",
            vector[0],
            vector[1]
        );
        assert!(
            (vector[1] - second).abs() < 1e-5,
            "{text}: {} {}",
            vector[0],
            vector[1]
        );
    }
}

const PINNED: [(f32, f32); 3] = [
    (-0.0028645368, -0.009736361),
    (0.23130728, 0.09866479),
    (0.030564217, -0.009908953),
];
