//! Netlist export.
//!
//! The schematic on the bench is not just a picture: `Circuit::nets()` already
//! knows which pins share a copper strip or a jumper. This module writes that
//! connectivity in the plain-text netlist format KiCad reads, so a build can
//! leave the drawing tool and be checked elsewhere.

use crate::model::Circuit;

fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A stable netlist of every live part. Roots are ordered by their first pin,
/// so the same circuit always produces the same file.
pub fn netlist(c: &Circuit) -> String {
    let nets = c.nets();
    let mut out = String::from("(export (version \"E\")\n  (nets\n");
    for (code, root) in nets.roots_sorted().into_iter().enumerate() {
        let name = c.net_name(&nets, root);
        out.push_str(&format!(
            "    (net (code {}) (name {})\n",
            code + 1,
            quote(&name)
        ));
        let mut nodes: Vec<String> = Vec::new();
        for &pin in nets.pins_in(root) {
            let Some(p) = c.pins.get(pin).filter(|p| p.active) else { continue };
            let Some(part) = c.parts.get(p.part).filter(|p| p.active) else { continue };
            nodes.push(format!(
                "      (node (ref {}) (pin {}))\n",
                quote(&part.label),
                quote(p.name)
            ));
        }
        nodes.sort();
        for node in nodes {
            out.push_str(&node);
        }
        out.push_str("    )\n");
    }
    out.push_str("  )\n  (components\n");
    let mut comps: Vec<String> = Vec::new();
    for part in c.parts() {
        comps.push(format!(
            "    (comp (ref {}) (value {}))\n",
            quote(&part.label),
            quote(&part.value)
        ));
    }
    comps.sort();
    for comp in comps {
        out.push_str(&comp);
    }
    out.push_str("  )\n)\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{End, Hole, NetClass, PinKind, PinSpec, Row};

    fn circuit() -> Circuit {
        let mut c = Circuit::new();
        c.board.at = crate::model::Pt::new(6.0, 6.0);
        c.add_arduino(crate::model::Pt::new(60.0, 20.0));
        let r = c.add_part(
            crate::model::Kind::Resistor,
            "R1",
            "220 Ω",
            crate::model::Pt::new(20.0, 16.0),
            vec![
                PinSpec { name: "1", pos: c.board.hole(5, Row::A), hole: Hole::Col(5, Row::A), kind: PinKind::Passive },
                PinSpec { name: "2", pos: c.board.hole(8, Row::A), hole: Hole::Col(8, Row::A), kind: PinKind::Passive },
            ],
            vec![],
        );
        let gnd = c.pin(0, "GND1");
        let res = c.pin(r, "1");
        c.wire(End::Pin(gnd), End::Pin(res), NetClass::Gnd);
        c
    }

    #[test]
    fn a_netlist_names_the_power_net_and_lists_the_parts() {
        let c = circuit();
        let text = netlist(&c);
        assert!(text.contains("(name \"GND\")"), "{text}");
        assert!(text.contains("(ref \"R1\") (pin \"1\")"), "{text}");
        assert!(text.contains("(comp (ref \"R1\") (value \"220 Ω\"))"), "{text}");

    }

    #[test]
    fn a_user_name_survives_another_wire_on_the_same_net() {
        let mut c = circuit();
        let nets = c.nets();
        let root = nets.root(c.pin(0, "GND1"));
        assert!(c.set_net_name(&nets, root, "AGND"));
        let spare = c.pin(0, "GND2");
        c.wire(
            End::Pin(spare),
            End::Hole(Hole::Col(9, Row::J)),
            NetClass::Gnd,
        );
        let nets = c.nets();
        assert_eq!(c.net_name(&nets, nets.root(spare)), "AGND");
        assert!(netlist(&c).contains("(name \"AGND\")"));
    }

    #[test]
    fn the_netlist_is_deterministic() {
        assert_eq!(netlist(&circuit()), netlist(&circuit()));
    }
}
