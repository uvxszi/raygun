use super::*;

fn math_of(html: &str) -> Formula {
    let document = scraper::Html::parse_fragment(html);
    let math = document
        .tree
        .root()
        .descendants()
        .find(|node| is_math(*node))
        .expect("a math element");
    Formula::from_element(math).expect("a formula")
}

#[test]
fn mathml_keeps_only_math_elements_and_safe_attributes() {
    let formula = math_of(
        r#"<math display="block" onclick="alert(1)" alttext="x">
            <mrow href="https://evil.example" style="background: url(https://evil.example/x)">
              <msup><mi mathvariant="normal" class="mjx-i">x</mi><mn>2</mn></msup>
              <mo>+</mo>
              <script>alert(2)</script>
              <a href="https://evil.example"><mn>1</mn></a>
              <mtext><img src="x" onerror="alert(3)">units</mtext>
            </mrow>
            <annotation-xml encoding="text/html"><p>leak</p></annotation-xml>
          </math>"#,
    );
    assert!(formula.display);
    assert_eq!(
        formula.mathml,
        "<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\"><mrow>\
         <msup><mi mathvariant=\"normal\">x</mi><mn>2</mn></msup><mo>+</mo><mn>1</mn>\
         <mtext>units</mtext></mrow></math>"
    );
    for forbidden in [
        "script", "onclick", "href", "url(", "img", "alert", "leak", "mjx-i",
    ] {
        assert!(
            !formula.mathml.contains(forbidden),
            "{forbidden} got through"
        );
    }
    assert_eq!(formula.text, "x² + 1units");
    assert_eq!(formula.speech, "x squared plus 1 units");
}

#[test]
fn deprecated_and_prefixed_mathml_is_rewritten() {
    // EPUB 2 books prefix MathML; mfenced isn't in MathML Core.
    let formula = math_of(
        r#"<m:math xmlns:m="http://www.w3.org/1998/Math/MathML"><m:mfenced><m:mi>a</m:mi><m:mi>b</m:mi></m:mfenced></m:math>"#,
    );
    assert_eq!(
        formula.mathml,
        "<math xmlns=\"http://www.w3.org/1998/Math/MathML\"><mrow><mo fence=\"true\">(</mo>\
         <mi>a</mi><mo>,</mo><mi>b</mi><mo fence=\"true\">)</mo></mrow></math>"
    );
    assert_eq!(formula.text, "(a, b)");
}

#[test]
fn wikipedia_formulas_keep_their_tex_as_text() {
    let formula = math_of(
        r#"<math xmlns="http://www.w3.org/1998/Math/MathML" alttext="{\displaystyle E=mc^{2}}"><semantics><mrow class="MJX-TeXAtom-ORD"><mstyle displaystyle="true" scriptlevel="0"><mi>E</mi><mo>=</mo><mi>m</mi><msup><mi>c</mi><mn>2</mn></msup></mstyle></mrow><annotation encoding="application/x-tex">{\displaystyle E=mc^{2}}</annotation></semantics></math>"#,
    );
    assert_eq!(formula.text, "E=mc^{2}");
    assert_eq!(formula.speech, "E equals m c squared");
    assert!(
        formula
            .mathml
            .contains("<annotation encoding=\"application/x-tex\">")
    );
}

#[test]
fn tex_becomes_mathml() {
    let formula = Formula::from_tex(r"\frac{a+b}{\sqrt{x}}", false).unwrap();
    assert!(formula.mathml.starts_with("<math xmlns="));
    assert!(formula.mathml.contains("<mfrac>"));
    assert!(formula.mathml.contains("<msqrt>"));
    assert_eq!(formula.text, r"\frac{a+b}{\sqrt{x}}");
    assert_eq!(
        formula.speech,
        "the fraction a plus b, over the square root of x, end fraction"
    );
    let display = Formula::from_tex(r"\sum_{i=1}^{n} i^2", true).unwrap();
    assert!(display.display);
    assert!(display.mathml.contains("display=\"block\""));
    assert_eq!(display.speech, "the sum from i equals 1 to n, of i squared");
    // MediaWiki's wrapper goes; what isn't TeX the converter reads stays text.
    assert_eq!(
        Formula::from_tex(r"{\displaystyle \alpha}", false)
            .unwrap()
            .text,
        r"\alpha"
    );
    assert!(Formula::from_tex(r"\frac{a", false).is_none());
    assert!(Formula::from_tex("   ", false).is_none());
}

fn tex<'a>(tex: &'a str, display: bool, source: &'a str) -> TextPiece<'a> {
    TextPiece::Tex {
        tex,
        display,
        source,
    }
}

#[test]
fn tex_in_text_is_found_by_its_delimiters() {
    let pieces = split_tex(r"Let $x^2$ cost \(y\) and $$z = 1$$ while \[w\] ends.");
    assert_eq!(
        pieces,
        [
            TextPiece::Text("Let "),
            tex("x^2", false, "$x^2$"),
            TextPiece::Text(" cost "),
            tex("y", false, r"\(y\)"),
            TextPiece::Text(" and "),
            tex("z = 1", true, "$$z = 1$$"),
            TextPiece::Text(" while "),
            tex("w", true, r"\[w\]"),
            TextPiece::Text(" ends."),
        ]
    );
}

#[test]
fn prices_and_escaped_dollars_stay_words() {
    for text in [
        "It cost $5 and $10 later.",
        "A $20,000 and $30,000 offer.",
        "Pay $ 5 or $ 6.",
        r"Escaped \$x\$ stays.",
        "Only one $dollar here.",
        "Between $hello world$ and more.",
    ] {
        assert_eq!(split_tex(text), [TextPiece::Text(text)], "{text}");
    }
    assert_eq!(
        split_tex("the function $f(x)$ grows"),
        [
            TextPiece::Text("the function "),
            tex("f(x)", false, "$f(x)$"),
            TextPiece::Text(" grows"),
        ]
    );
}

#[test]
fn letters_from_math_alphabets_read_plainly() {
    assert_eq!(speech::plain("𝑥 + 𝐀 = 𝟏ℎ"), "x + A = 1h");
    assert_eq!(speech::raised("2"), "²");
    assert_eq!(speech::raised("n+1"), "ⁿ⁺¹");
    assert_eq!(speech::raised("k+1"), "^(k+1)");
    assert_eq!(speech::lowered("12"), "₁₂");
}

#[test]
fn tex_spaces_are_spaces_not_entity_text() {
    for tex in [r"3.1\ \mathrm{miles}", r"a~b", r"5\text{ km}"] {
        let formula = Formula::from_tex(tex, false).unwrap();
        assert!(
            !formula.mathml.contains("nbsp"),
            "{tex}: {}",
            formula.mathml
        );
        assert!(
            !formula.speech.contains("nbsp"),
            "{tex}: {}",
            formula.speech
        );
    }
}
