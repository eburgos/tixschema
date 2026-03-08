#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test file")]
mod tests {

    #[cfg(all(
        test,
        any(
            feature = "typescript",
            feature = "jsonschema",
            feature = "zod",
            feature = "serde"
        )
    ))]
    use tixschema::model_schema;

    #[cfg(all(test, feature = "serde"))]
    #[cfg(all(test, feature = "serde"))]
    use serde::{Deserialize, Serialize};
    #[cfg(all(test, feature = "jsonschema", feature = "serde"))]
    use serde_json::Value;

    #[cfg(all(test, any(feature = "typescript", feature = "zod", feature = "serde")))]
    #[model_schema()]
    #[cfg_attr(
        feature = "serde",
        derive(Serialize, Deserialize),
        serde(rename_all = "lowercase")
    )]
    #[derive(Debug, Clone, PartialEq)]
    enum UserStatus {
        Active,
        Inactive,
        Pending,
        Suspended,
    }

    #[test]
    #[cfg(all(feature = "jsonschema", feature = "serde"))]
    fn test_plain_enum_json_schema() {
        let schema = UserStatus::json_schema();

        assert_eq!(schema["type"], "string");

        let enum_values = schema["enum"].as_array().unwrap();
        assert_eq!(enum_values.len(), 4);
        assert!(enum_values.contains(&Value::String("active".to_string())));
        assert!(enum_values.contains(&Value::String("inactive".to_string())));
        assert!(enum_values.contains(&Value::String("pending".to_string())));
        assert!(enum_values.contains(&Value::String("suspended".to_string())));
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "serde", feature = "zod"))]
    fn test_plain_enum_ts_definition_serde_style() {
        let ts_definition = UserStatus::ts_definition();
        // println!("TypeScript output:\n{ts_definition}");

        // Check TypeScript union type
        assert!(ts_definition.contains("export type UserStatus"));
        assert!(ts_definition.contains("\"active\""));
        assert!(ts_definition.contains("\"inactive\""));
        assert!(ts_definition.contains("\"pending\""));
        assert!(ts_definition.contains("\"suspended\""));

        // Check Zod schema - now in separate method
        let zod_schema = UserStatus::zod_schema();
        assert!(zod_schema.contains("export const UserStatus$Schema"));
        assert!(
            zod_schema.contains("z.enum([\"active\", \"inactive\", \"pending\", \"suspended\"])")
        );
    }

    #[test]
    #[cfg(all(feature = "typescript", not(feature = "serde"), feature = "zod"))]
    fn test_plain_enum_ts_definition_not_serde_style() {
        let ts_definition = UserStatus::ts_definition();

        // Check TypeScript union type
        assert!(ts_definition.contains("export type UserStatus"));
        assert!(ts_definition.contains("\"Active\""));
        assert!(ts_definition.contains("\"Inactive\""));
        assert!(ts_definition.contains("\"Pending\""));
        assert!(ts_definition.contains("\"Suspended\""));

        // Check Zod schema - now in separate method
        let zod_schema = UserStatus::zod_schema();
        assert!(zod_schema.contains("export const UserStatus$Schema"));
        assert!(
            zod_schema.contains("z.enum([\"Active\", \"Inactive\", \"Pending\", \"Suspended\"])")
        );
    }

    #[test]
    #[cfg(all(
        any(feature = "typescript", feature = "zod", feature = "jsonschema"),
        feature = "serde"
    ))]
    fn test_plain_enum_members() {
        let members = UserStatus::enum_members();
        assert_eq!(members.len(), 4);
        assert!(members.contains(&"active".to_string()));
        assert!(members.contains(&"inactive".to_string()));
        assert!(members.contains(&"pending".to_string()));
        assert!(members.contains(&"suspended".to_string()));
    }

    #[cfg(all(
        test,
        any(feature = "typescript", feature = "jsonschema", feature = "zod")
    ))]
    // Test discriminated union (tagged enum)
    #[model_schema()]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, PartialEq)]
    #[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "camelCase"))]
    enum PaymentMethod {
        CreditCard {
            card_number: String,
            expiry_date: String,
            cvv: String,
        },
        BankTransfer {
            account_number: String,
            routing_number: String,
        },
        PayPal {
            email: String,
        },
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_discriminated_union_json_schema() {
        let schema = PaymentMethod::json_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema.get("oneOf").is_some());

        let one_of = schema["oneOf"].as_array().unwrap();
        assert_eq!(one_of.len(), 3);

        // Check that each variant has the discriminator field
        for variant in one_of {
            let properties = variant["properties"].as_object().unwrap();
            assert!(properties.contains_key("type"));
            assert_eq!(properties["type"]["type"], "string");
            assert!(properties["type"].get("const").is_some());
        }
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_payment_method_variants_json_schema() {
        let payment_method = PaymentMethod::PayPal {
            email: "test@test.com".to_string(),
        };
        assert_ne!(Some(payment_method), None);

        let payment_method_2 = PaymentMethod::CreditCard {
            card_number: "1234567890".to_string(),
            expiry_date: "12/2025".to_string(),
            cvv: "123".to_string(),
        };
        assert_ne!(Some(payment_method_2), None);

        let payment_method_3 = PaymentMethod::BankTransfer {
            account_number: "1234567890".to_string(),
            routing_number: "1234567890".to_string(),
        };
        assert_ne!(Some(payment_method_3), None);
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "serde", feature = "zod"))]
    fn test_discriminated_union_ts_definition() {
        let ts_definition = PaymentMethod::ts_definition();

        // Check that it contains discriminated union syntax
        assert!(ts_definition.contains("export type PaymentMethod = "));
        assert!(ts_definition.contains("type: \"creditCard\""));
        assert!(ts_definition.contains("type: \"bankTransfer\""));
        assert!(ts_definition.contains("type: \"payPal\""));

        // Check field names are converted to camelCase
        assert!(ts_definition.contains("cardNumber: string;"));
        assert!(ts_definition.contains("expiryDate: string;"));
        assert!(ts_definition.contains("accountNumber: string;"));
        assert!(ts_definition.contains("routingNumber: string;"));

        // Check Zod discriminated union - now in separate method
        let zod_schema = PaymentMethod::zod_schema();
        assert!(zod_schema.contains("z.discriminatedUnion(\"type\""));
    }

    #[cfg(all(test, any(feature = "typescript", feature = "zod", feature = "serde")))]
    /**
     * Calculated Expression Operator
     *
     * Represents the operator to be used in a calculated expression.
     */
    #[model_schema()]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum CalculatedExpressionOperator {
        /**
         * No operation. Commonly used with single value summaries
         */
        None,
        /**
         * Addition. Adds a value with another value.
         */
        Add,
        /**
         * Subtraction. Subtracts a value from another value.
         */
        Subtract,
        /**
         * Multiplication. The product of two values.
         */
        Multiply,
        /**
         * Division. Divide a value by another value.
         */
        Divide,
        /**
         * Modulus. The modulus result of an integer division operation.
         */
        Modulus,
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "serde", feature = "zod"))]
    fn test_enum_with_docs() {
        let ts_definition = CalculatedExpressionOperator::ts_definition();
        // println!("=== TypeScript Definition ===\n{ts_definition}");

        // let zod_schema = CalculatedExpressionOperator::zod_schema();
        // println!("\n=== Zod Schema ===\n{zod_schema}");

        // Just make sure it compiles for now
        assert!(ts_definition.contains("export type CalculatedExpressionOperator"));
    }

    // =============================================
    // Single-value enum as literal type alternative
    // =============================================
    // Tests the recommended pattern of using single-value enums
    // instead of #[model_schema_prop(literal = "value")] on String fields.
    // This provides type safety in Rust while generating identical TypeScript output.

    #[cfg(all(
        test,
        any(feature = "typescript", feature = "zod", feature = "serde")
    ))]
    #[model_schema()]
    #[cfg_attr(
        feature = "serde",
        derive(Serialize, Deserialize),
        serde(rename_all = "lowercase")
    )]
    #[derive(Debug, Clone, PartialEq)]
    enum DocumentLiteralValue {
        Document,
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "serde", feature = "zod"))]
    fn test_single_value_enum_ts_definition() {
        let ts_definition = DocumentLiteralValue::ts_definition();

        // A single-value enum should generate a literal union type
        assert!(ts_definition.contains("export type DocumentLiteralValue"));
        assert!(ts_definition.contains("\"document\""));
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "serde", feature = "zod"))]
    fn test_single_value_enum_zod_schema() {
        let zod_schema = DocumentLiteralValue::zod_schema();

        // Should generate z.enum(["document"]) which is equivalent to z.literal("document")
        assert!(zod_schema.contains("export const DocumentLiteralValue$Schema"));
        assert!(zod_schema.contains("z.enum([\"document\"])"));
    }

    // Test single-value enum used as a field in a discriminated union
    #[cfg(all(
        test,
        any(feature = "typescript", feature = "zod", feature = "serde")
    ))]
    #[model_schema()]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, PartialEq)]
    #[cfg_attr(feature = "serde", serde(tag = "source"))]
    enum ActionWithLiteralEnum {
        #[cfg_attr(feature = "serde", serde(rename = "generate"))]
        Generate { value: DocumentLiteralValue },
        #[cfg_attr(feature = "serde", serde(rename = "upload"))]
        Upload { value: String },
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "serde", feature = "zod"))]
    fn test_single_value_enum_in_tagged_union_ts() {
        let ts_definition = ActionWithLiteralEnum::ts_definition();

        // The generate variant should have value typed as DocumentLiteralValue
        assert!(ts_definition.contains("value: DocumentLiteralValue;"));
        // The upload variant should have value typed as string
        assert!(ts_definition.contains("value: string;"));
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "serde", feature = "zod"))]
    fn test_single_value_enum_in_tagged_union_zod() {
        let zod_schema = ActionWithLiteralEnum::zod_schema();

        // The generate variant should reference DocumentLiteralValue$Schema
        assert!(zod_schema.contains("DocumentLiteralValue$Schema"));
        // The upload variant should use z.string()
        assert!(zod_schema.contains("z.string()"));
    }
}
