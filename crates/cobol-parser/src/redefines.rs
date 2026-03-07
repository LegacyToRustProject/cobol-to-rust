/// REDEFINES clause conversion: generates Rust structs and `From` trait impls.
///
/// COBOL REDEFINES allows multiple interpretations of the same memory:
/// ```text
/// 01 WS-BUFFER PIC X(10).
/// 01 WS-NAME REDEFINES WS-BUFFER.
///    05 WS-FIRST PIC X(5).
///    05 WS-LAST  PIC X(5).
/// ```
///
/// The Rust equivalent uses a byte-array buffer struct + `From` trait:
/// ```text
/// struct WsBuffer([u8; 10]);
/// struct WsName { ws_first: String, ws_last: String }
/// impl From<WsBuffer> for WsName { ... }
/// ```
use crate::types::{DataItem, PicClause, PicType};

/// Represents one field in a REDEFINES target struct.
#[derive(Debug, Clone, PartialEq)]
pub struct RedefinesField {
    pub name: String,
    /// Byte offset within the parent buffer
    pub offset: usize,
    /// Byte length of this field
    pub length: usize,
    pub field_type: RedefinesFieldType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RedefinesFieldType {
    /// PIC X(N) — alphanumeric → String
    Alphanumeric,
    /// PIC 9(N) — numeric integer → u64 / i64
    Numeric { signed: bool },
    /// PIC 9(N)V99 — decimal → Decimal
    Decimal { decimal_digits: u32 },
}

/// Complete description of a REDEFINES relationship.
#[derive(Debug, Clone)]
pub struct RedefinesSpec {
    /// The base variable being redefined (e.g., "WS-BUFFER")
    pub base_name: String,
    /// Total byte size of the base variable
    pub base_size: usize,
    /// The name of the redefining group (e.g., "WS-NAME")
    pub alias_name: String,
    /// Fields within the alias interpretation
    pub fields: Vec<RedefinesField>,
}

impl RedefinesSpec {
    /// Generate the Rust buffer struct declaration.
    /// e.g. `struct WsBuffer([u8; 10]);`
    pub fn buffer_struct_decl(&self) -> String {
        let struct_name = to_rust_type_name(&self.base_name);
        format!(
            "#[derive(Debug, Clone)]\nstruct {}([u8; {}]);",
            struct_name, self.base_size
        )
    }

    /// Generate the Rust alias struct declaration.
    pub fn alias_struct_decl(&self) -> String {
        let struct_name = to_rust_type_name(&self.alias_name);
        let fields: String = self
            .fields
            .iter()
            .map(|f| {
                let rust_type = match &f.field_type {
                    RedefinesFieldType::Alphanumeric => "String",
                    RedefinesFieldType::Numeric { signed: true } => "i64",
                    RedefinesFieldType::Numeric { signed: false } => "u64",
                    RedefinesFieldType::Decimal { .. } => "rust_decimal::Decimal",
                };
                format!("    pub {}: {},", to_rust_field_name(&f.name), rust_type)
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "#[derive(Debug, Clone)]\nstruct {} {{\n{}\n}}",
            struct_name, fields
        )
    }

    /// Generate the `From<BaseStruct> for AliasStruct` impl.
    pub fn from_impl(&self) -> String {
        let buf_type = to_rust_type_name(&self.base_name);
        let alias_type = to_rust_type_name(&self.alias_name);
        let field_inits: String = self
            .fields
            .iter()
            .map(|f| {
                let start = f.offset;
                let end = f.offset + f.length;
                let rust_field = to_rust_field_name(&f.name);
                match &f.field_type {
                    RedefinesFieldType::Alphanumeric => format!(
                        "            {}: String::from_utf8_lossy(&buf.0[{}..{}]).trim().to_string(),",
                        rust_field, start, end
                    ),
                    RedefinesFieldType::Numeric { .. } => format!(
                        "            {}: String::from_utf8_lossy(&buf.0[{}..{}]).trim().parse().unwrap_or(0),",
                        rust_field, start, end
                    ),
                    RedefinesFieldType::Decimal { decimal_digits } => format!(
                        "            {}: {{\n                let raw = String::from_utf8_lossy(&buf.0[{}..{}]);\n                let raw = raw.trim();\n                let (int_p, dec_p) = raw.split_at(raw.len().saturating_sub({}));\n                rust_decimal::Decimal::from_str_exact(&format!(\"{{}}.{{}}\", int_p.trim_start_matches('0').replace(\"\", \"0\"), dec_p)).unwrap_or_default()\n            }},",
                        rust_field, start, end, decimal_digits
                    ),
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "impl From<{}> for {} {{\n    fn from(buf: {}) -> Self {{\n        {} {{\n{}\n        }}\n    }}\n}}",
            buf_type, alias_type, buf_type, alias_type, field_inits
        )
    }

    /// Generate all three pieces (buffer struct, alias struct, From impl).
    pub fn generate_all(&self) -> String {
        format!(
            "{}\n\n{}\n\n{}",
            self.buffer_struct_decl(),
            self.alias_struct_decl(),
            self.from_impl()
        )
    }
}

/// Convert a COBOL hyphenated name to a Rust CamelCase type name.
pub fn to_rust_type_name(cobol_name: &str) -> String {
    cobol_name
        .split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str().to_lowercase().as_str(),
            }
        })
        .collect()
}

/// Convert a COBOL hyphenated name to a Rust snake_case field name.
pub fn to_rust_field_name(cobol_name: &str) -> String {
    cobol_name.replace('-', "_").to_lowercase()
}

/// Compute the byte size of a PIC clause.
pub fn pic_byte_size(pic: &PicClause) -> usize {
    pic.total_size as usize
}

/// Extract REDEFINES relationships from a list of DataItems.
///
/// Returns one `RedefinesSpec` per REDEFINES clause found.
pub fn extract_redefines(items: &[DataItem]) -> Vec<RedefinesSpec> {
    let mut specs = Vec::new();

    for item in items {
        if let Some(ref base_name) = item.redefines {
            // Find the base item to get its size
            let base_size = items
                .iter()
                .find(|i| i.name == *base_name)
                .and_then(|i| i.picture.as_ref())
                .map(pic_byte_size)
                .unwrap_or(0);

            // Build fields from children
            let mut offset = 0usize;
            let fields: Vec<RedefinesField> = item
                .children
                .iter()
                .filter_map(|child| {
                    let pic = child.picture.as_ref()?;
                    let length = pic_byte_size(pic);
                    let field_type = match pic.pic_type {
                        PicType::Alphanumeric | PicType::Alphabetic => {
                            RedefinesFieldType::Alphanumeric
                        }
                        PicType::Numeric if pic.decimal_digits > 0 => RedefinesFieldType::Decimal {
                            decimal_digits: pic.decimal_digits,
                        },
                        PicType::Numeric => RedefinesFieldType::Numeric { signed: pic.signed },
                    };
                    let f = RedefinesField {
                        name: child.name.clone(),
                        offset,
                        length,
                        field_type,
                    };
                    offset += length;
                    Some(f)
                })
                .collect();

            specs.push(RedefinesSpec {
                base_name: base_name.clone(),
                base_size,
                alias_name: item.name.clone(),
                fields,
            });
        }
    }

    specs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PicClause, PicType};

    fn alphanumeric_pic(size: u32) -> PicClause {
        PicClause {
            raw: format!("X({})", size),
            pic_type: PicType::Alphanumeric,
            integer_digits: 0,
            decimal_digits: 0,
            signed: false,
            total_size: size,
        }
    }

    fn numeric_pic(int_digits: u32, dec_digits: u32, signed: bool) -> PicClause {
        PicClause {
            raw: format!("9({})", int_digits),
            pic_type: PicType::Numeric,
            integer_digits: int_digits,
            decimal_digits: dec_digits,
            signed,
            total_size: int_digits + dec_digits,
        }
    }

    fn make_item(
        name: &str,
        level: u8,
        picture: Option<PicClause>,
        redefines: Option<&str>,
        children: Vec<DataItem>,
    ) -> DataItem {
        DataItem {
            level,
            name: name.to_string(),
            picture,
            value: None,
            redefines: redefines.map(|s| s.to_string()),
            children,
        }
    }

    #[test]
    fn test_to_rust_type_name() {
        assert_eq!(to_rust_type_name("WS-BUFFER"), "WsBuffer");
        assert_eq!(to_rust_type_name("INPUT-RECORD"), "InputRecord");
        assert_eq!(to_rust_type_name("TRANS-FILE"), "TransFile");
    }

    #[test]
    fn test_to_rust_field_name() {
        assert_eq!(to_rust_field_name("WS-FIRST-NAME"), "ws_first_name");
        assert_eq!(to_rust_field_name("TR-AMOUNT"), "tr_amount");
    }

    #[test]
    fn test_buffer_struct_decl() {
        let spec = RedefinesSpec {
            base_name: "WS-BUFFER".to_string(),
            base_size: 10,
            alias_name: "WS-NAME".to_string(),
            fields: vec![],
        };
        let decl = spec.buffer_struct_decl();
        assert!(decl.contains("WsBuffer"), "should use CamelCase type name");
        assert!(
            decl.contains("[u8; 10]"),
            "should declare correct byte array size"
        );
    }

    #[test]
    fn test_alias_struct_decl_alphanumeric() {
        let spec = RedefinesSpec {
            base_name: "WS-BUFFER".to_string(),
            base_size: 10,
            alias_name: "WS-NAME".to_string(),
            fields: vec![
                RedefinesField {
                    name: "WS-FIRST".to_string(),
                    offset: 0,
                    length: 5,
                    field_type: RedefinesFieldType::Alphanumeric,
                },
                RedefinesField {
                    name: "WS-LAST".to_string(),
                    offset: 5,
                    length: 5,
                    field_type: RedefinesFieldType::Alphanumeric,
                },
            ],
        };
        let decl = spec.alias_struct_decl();
        assert!(
            decl.contains("WsName"),
            "should use CamelCase alias type name"
        );
        assert!(
            decl.contains("ws_first: String"),
            "alphanumeric field should be String"
        );
        assert!(
            decl.contains("ws_last: String"),
            "alphanumeric field should be String"
        );
    }

    #[test]
    fn test_from_impl_generated() {
        let spec = RedefinesSpec {
            base_name: "WS-BUF".to_string(),
            base_size: 6,
            alias_name: "WS-PARTS".to_string(),
            fields: vec![
                RedefinesField {
                    name: "WS-A".to_string(),
                    offset: 0,
                    length: 3,
                    field_type: RedefinesFieldType::Alphanumeric,
                },
                RedefinesField {
                    name: "WS-B".to_string(),
                    offset: 3,
                    length: 3,
                    field_type: RedefinesFieldType::Alphanumeric,
                },
            ],
        };
        let impl_code = spec.from_impl();
        assert!(
            impl_code.contains("impl From<WsBuf> for WsParts"),
            "should have From impl"
        );
        assert!(
            impl_code.contains("from_utf8_lossy(&buf.0[0..3])"),
            "should use correct byte slice for ws_a"
        );
        assert!(
            impl_code.contains("from_utf8_lossy(&buf.0[3..6])"),
            "should use correct byte slice for ws_b"
        );
    }

    #[test]
    fn test_extract_redefines_from_data_items() {
        let base = make_item("WS-BUFFER", 1, Some(alphanumeric_pic(10)), None, vec![]);
        let child1 = make_item("WS-FIRST", 5, Some(alphanumeric_pic(5)), None, vec![]);
        let child2 = make_item("WS-LAST", 5, Some(alphanumeric_pic(5)), None, vec![]);
        let alias = make_item("WS-NAME", 1, None, Some("WS-BUFFER"), vec![child1, child2]);
        let items = vec![base, alias];
        let specs = extract_redefines(&items);
        assert_eq!(specs.len(), 1);
        let spec = &specs[0];
        assert_eq!(spec.base_name, "WS-BUFFER");
        assert_eq!(spec.base_size, 10);
        assert_eq!(spec.alias_name, "WS-NAME");
        assert_eq!(spec.fields.len(), 2);
        assert_eq!(spec.fields[0].offset, 0);
        assert_eq!(spec.fields[0].length, 5);
        assert_eq!(spec.fields[1].offset, 5);
        assert_eq!(spec.fields[1].length, 5);
    }

    #[test]
    fn test_generate_all_produces_complete_code() {
        let spec = RedefinesSpec {
            base_name: "WS-RECORD".to_string(),
            base_size: 20,
            alias_name: "WS-PARSED".to_string(),
            fields: vec![RedefinesField {
                name: "WS-CODE".to_string(),
                offset: 0,
                length: 20,
                field_type: RedefinesFieldType::Alphanumeric,
            }],
        };
        let code = spec.generate_all();
        assert!(code.contains("WsRecord([u8; 20])"), "buffer struct");
        assert!(code.contains("struct WsParsed"), "alias struct");
        assert!(
            code.contains("impl From<WsRecord> for WsParsed"),
            "From impl"
        );
    }
}
