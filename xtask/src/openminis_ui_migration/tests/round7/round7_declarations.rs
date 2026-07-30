#[test]
fn openminis_ui_migration_declarations_reject_lexical_forgeries() {
    let source = r#"
        // function forgedSymbol() {}
        const text = "function forgedSymbol() {}";
        const pattern = /function regexForgedSymbol/;
        forgedSymbol();
        function forgedSymbolSuffix() {}
        function localScope() {
            const localForgedSymbol = () => null;
            function nestedForgedSymbol() {}
        }
        for (let loopForgedSymbol = 0; loopForgedSymbol < 1; loopForgedSymbol++) {}
        if (true) function branchForgedSymbol() {}
    "#;
    let declarations =
        declared_symbols(Path::new("fixture.js"), source).expect("parse JavaScript declarations");
    assert!(
        !declarations
            .iter()
            .any(|declaration| declaration.name == "forgedSymbol")
    );
    assert!(
        !declarations
            .iter()
            .any(|declaration| declaration.name == "regexForgedSymbol")
    );
    assert!(
        declarations
            .iter()
            .any(|declaration| declaration.name == "forgedSymbolSuffix")
    );
    assert!(
        !declarations
            .iter()
            .any(|declaration| declaration.name == "localForgedSymbol")
    );
    assert!(
        !declarations
            .iter()
            .any(|declaration| declaration.name == "nestedForgedSymbol")
    );
    for local in ["loopForgedSymbol", "branchForgedSymbol"] {
        assert!(
            !declarations
                .iter()
                .any(|declaration| declaration.name == local),
            "control-flow local declaration `{local}` must not satisfy migration truth"
        );
    }

    let rust = r#"
        fn outer() {
            fn LocalForgedSymbol() {}
        }
        fn SurfaceSymbol() {}
    "#;
    let declarations =
        declared_symbols(Path::new("fixture.rs"), rust).expect("parse Rust declarations");
    assert!(
        !declarations
            .iter()
            .any(|declaration| declaration.name == "LocalForgedSymbol")
    );
    assert!(
        declarations
            .iter()
            .any(|declaration| declaration.name == "SurfaceSymbol")
    );

    let swift = r#"
        // struct Forged {}
        let text = "struct Forged {}"
        struct ForgedSuffix {
            let surfaceProperty: Int
            func surfaceMethod() {
                func LocalForgedSymbol() {}
                let localForgedValue = 1
            }
        }
    "#;
    let declarations =
        declared_symbols(Path::new("Fixture.swift"), swift).expect("parse Swift declarations");
    assert!(
        !declarations
            .iter()
            .any(|declaration| declaration.name == "Forged")
    );
    assert!(
        declarations
            .iter()
            .any(|declaration| declaration.name == "ForgedSuffix")
    );
    for local in ["LocalForgedSymbol", "localForgedValue"] {
        assert!(
            !declarations
                .iter()
                .any(|declaration| declaration.name == local),
            "Swift local declaration `{local}` must not satisfy migration truth"
        );
    }
    for surface in ["surfaceProperty", "surfaceMethod"] {
        assert!(
            declarations
                .iter()
                .any(|declaration| declaration.name == surface),
            "Swift surface declaration `{surface}` must remain visible"
        );
    }
}
use super::*;
