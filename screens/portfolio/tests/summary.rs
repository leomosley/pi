use screen_api::Screen;
use screen_portfolio::{PortfolioScreen, PortfolioSettings, parse_summary};

#[test]
fn uses_total_value_not_cost_or_profit() {
    let value = parse_summary(br#"{"currency":"GBP","totalValue":12345.67,"investments":{"currentValue":9000,"totalCost":8000,"unrealizedProfitLoss":1000}}"#).unwrap();
    assert_eq!(value.amount, "12,345.67");
}
#[test]
fn preserves_primary_currency() {
    assert_eq!(
        parse_summary(br#"{"currency":"EUR","totalValue":12}"#)
            .unwrap()
            .currency,
        "EUR"
    );
}
#[test]
fn zero_is_a_valid_account_value() {
    assert_eq!(
        parse_summary(br#"{"currency":"GBP","totalValue":0}"#)
            .unwrap()
            .amount,
        "0.00"
    );
}
#[test]
fn formats_zero_decimal_currency() {
    assert_eq!(
        parse_summary(br#"{"currency":"JPY","totalValue":12345}"#)
            .unwrap()
            .amount,
        "12,345"
    );
}
#[test]
fn missing_value_is_not_zero() {
    assert!(parse_summary(br#"{"currency":"GBP"}"#).is_err());
}
#[test]
fn negative_value_is_rejected() {
    assert!(parse_summary(br#"{"currency":"GBP","totalValue":-1}"#).is_err());
}
#[test]
fn unbounded_value_is_rejected() {
    assert!(parse_summary(br#"{"currency":"GBP","totalValue":1e100}"#).is_err());
}
#[test]
fn currency_control_sequences_are_rejected() {
    assert!(parse_summary(br#"{"currency":"\u001b[0m","totalValue":1}"#).is_err());
}
#[test]
fn numeric_strings_are_rejected() {
    assert!(parse_summary(br#"{"currency":"GBP","totalValue":"1"}"#).is_err());
}
#[test]
fn errors_do_not_reveal_response_body() {
    assert!(
        !parse_summary(b"private account secret")
            .unwrap_err()
            .to_string()
            .contains("private")
    );
}
#[test]
fn invalid_refresh_interval_is_rejected() {
    assert!(
        PortfolioScreen::new(PortfolioSettings {
            refresh_seconds: 1,
            ..Default::default()
        })
        .is_err()
    );
}
#[test]
fn invalid_environment_name_is_rejected() {
    assert!(
        PortfolioScreen::new(PortfolioSettings {
            key_env: "key=secret".into(),
            ..Default::default()
        })
        .is_err()
    );
}
#[test]
fn demo_has_fixed_sample_value() {
    let mut screen = PortfolioScreen::new(PortfolioSettings {
        demo: true,
        ..Default::default()
    })
    .unwrap();
    screen.activate();
    screen.tick();
    assert_eq!(screen.amount(), "12,345.67");
}
#[test]
fn unavailable_value_is_not_fake_zero() {
    let screen = PortfolioScreen::new(PortfolioSettings::default()).unwrap();
    assert_eq!(screen.amount(), "--");
}

#[test]
fn gbp_display_includes_pound_symbol() {
    let screen = PortfolioScreen::new(PortfolioSettings {
        demo: true,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(screen.display_amount(), "£12,345.67");
}

#[test]
fn ascii_frame_has_no_brand_heading() {
    let screen = PortfolioScreen::new(PortfolioSettings {
        demo: true,
        ..Default::default()
    })
    .unwrap();
    let glyphs = screen_portfolio::portfolio_glyphs(ansi_renderer::GlyphLibrary::preset(
        ansi_renderer::GlyphSet::Ascii,
    ))
    .unwrap();
    assert!(!screen.ansi_frame(60, 40, &glyphs).contains("Trading 212"));
}

#[test]
fn pound_has_a_real_art_glyph() {
    let glyphs = screen_portfolio::portfolio_glyphs(ansi_renderer::GlyphLibrary::preset(
        ansi_renderer::GlyphSet::Ascii,
    ))
    .unwrap();
    assert_eq!(glyphs.glyph('£').rows()[0], "  ###");
}
