use blinc_charts_desktop::gallery::coverage_matrix;

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn main() {
    println!("{{\"cases\":[");
    for (index, case) in coverage_matrix().into_iter().enumerate() {
        if index > 0 {
            println!(",");
        }
        print!(
            concat!(
                "{{\"index\":{},\"family\":{},\"variant\":{},\"variant_code\":{},",
                "\"variant_effect\":{},\"interaction\":{},\"interaction_code\":{},",
                "\"interaction_effect\":{},\"task\":{},\"evidence\":{}}}"
            ),
            index,
            json_string(&format!("{:?}", case.family)),
            json_string(case.variant),
            json_string(case.variant_code),
            json_string(case.variant_effect),
            json_string(case.interaction),
            json_string(case.interaction_code),
            json_string(case.interaction_effect),
            json_string(&case.task),
            json_string(&case.evidence)
        );
    }
    println!("]}}");
}
