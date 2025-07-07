#[cfg(test)]
mod tests {

    #[cfg(all(
        test,
        any(feature = "typescript", feature = "jsonschema", feature = "zod", feature = "serde")
    ))]
    use tixschema::model_schema;

    #[cfg(all(test, feature = "serde"))]
    use serde;
    #[cfg(all(test, feature = "serde"))]
    use serde::{Deserialize, Serialize};
    #[cfg(all(test, feature = "jsonschema", feature = "serde"))]
    use serde_json::Value;

    #[cfg(all(
        test,
        any(
            feature = "typescript",
            feature = "zod",
            feature = "serde"
        )
    ))]
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

        // Check TypeScript union type
        assert!(ts_definition.contains("export type UserStatus = "));
        assert!(ts_definition.contains("\"active\" | \"inactive\" | \"pending\" | \"suspended\""));

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
        assert!(ts_definition.contains("export type UserStatus = "));
        assert!(ts_definition.contains("\"Active\" | \"Inactive\" | \"Pending\" | \"Suspended\""));

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
        
        let payment_method = PaymentMethod::PayPal { email: "test@test.com".to_string() };
        assert_ne!(Some(payment_method), None);
        
        let payment_method = PaymentMethod::CreditCard { card_number: "1234567890".to_string(), expiry_date: "12/2025".to_string(), cvv: "123".to_string() };
        assert_ne!(Some(payment_method), None);

        let payment_method = PaymentMethod::BankTransfer { account_number: "1234567890".to_string(), routing_number: "1234567890".to_string() };
        assert_ne!(Some(payment_method), None);

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
}
