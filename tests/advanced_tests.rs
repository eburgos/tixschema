use core_model_macros::model_schema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[cfg(test)]
mod advanced_tests {
    use super::*;

    // Test complex nested structures (fixed to work with actual macro)
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct CompanyJson {
        id: String,
        name: String,
        employees: Vec<EmployeeJson>,
        // Changed to string keys only - HashMap<String, Department> not supported
        department_names: Vec<String>,
        headquarters: AddressJson,
        settings: CompanySettingsJson,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct EmployeeJson {
        id: String,
        name: String,
        position: String,
        salary: u32,
        manager: Option<String>, // Manager ID
        skills: Vec<String>,
        contact: ContactInfoJson,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct ProjectJson {
        id: String,
        name: String,
        status: ProjectStatusJson,
        assigned_employees: Vec<String>,
        deadline: Option<String>,
        budget: Option<u32>,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    #[serde(rename_all = "camelCase")]
    enum ProjectStatusJson {
        NotStarted,
        InProgress,
        OnHold,
        Completed,
        Cancelled,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct ContactInfoJson {
        email: String,
        phone: Option<String>,
        emergency_contact: Option<EmergencyContactJson>,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct EmergencyContactJson {
        name: String,
        relationship: String,
        phone: String,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct AddressJson {
        street: String,
        city: String,
        state: String,
        zip_code: String,
        country: String,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    #[serde(rename_all = "camelCase")]
    struct CompanySettingsJson {
        allow_remote_work: bool,
        max_vacation_days: u32,
        health_insurance_provider: Option<String>,
        retirement_plan: Option<RetirementPlanJson>,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    #[serde(tag = "type", rename_all = "camelCase")]
    enum RetirementPlanJson {
        Option401k {
            employer_match_percentage: f32,
            vesting_schedule: String,
        },
        Pension {
            years_of_service_required: u32,
            monthly_benefit_multiplier: f32,
        },
        Roth {
            contribution_limit: u32,
            employer_contribution: bool,
        },
    }

    #[test]
    fn test_complex_nested_json_schema() {
        let company_schema = CompanyJson::json_schema();
        let employee_schema = EmployeeJson::json_schema();
        let project_schema = ProjectJson::json_schema();
        let contact_schema = ContactInfoJson::json_schema();
        let settings_schema = CompanySettingsJson::json_schema();
        let retirement_schema = RetirementPlanJson::json_schema();

        // Verify all schemas are objects
        assert_eq!(company_schema["type"], "object");
        assert_eq!(employee_schema["type"], "object");
        assert_eq!(project_schema["type"], "object");
        assert_eq!(contact_schema["type"], "object");
        assert_eq!(settings_schema["type"], "object");
        assert_eq!(retirement_schema["type"], "object");

        // Verify Company schema has all expected properties
        let company_properties = company_schema["properties"].as_object().unwrap();
        assert!(company_properties.contains_key("id"));
        assert!(company_properties.contains_key("name"));
        assert!(company_properties.contains_key("employees"));
        assert!(company_properties.contains_key("department_names"));
        assert!(company_properties.contains_key("headquarters"));
        assert!(company_properties.contains_key("settings"));

        // Verify array properties are correctly typed
        assert_eq!(company_properties["employees"]["type"], "array");
        assert_eq!(company_properties["department_names"]["type"], "array");
        assert_eq!(company_properties["department_names"]["items"]["type"], "string");

        // Verify RetirementPlan is a discriminated union
        assert!(retirement_schema.get("oneOf").is_some());
        let one_of = retirement_schema["oneOf"].as_array().unwrap();
        assert_eq!(one_of.len(), 3);
    }

    #[test]
    fn test_complex_nested_ts_definition() {
        let company_definition = CompanyJson::ts_definition();
        let employee_definition = EmployeeJson::ts_definition();
        let retirement_definition = RetirementPlanJson::ts_definition();

        // Check that nested types are properly referenced (without Json suffix)
        assert!(company_definition.contains("employees: Array<Employee>;"));
        assert!(company_definition.contains("department_names: Array<string>;"));
        assert!(company_definition.contains("headquarters: Address;"));
        assert!(company_definition.contains("settings: CompanySettings;"));

        // Check Zod schema references (without Json suffix)
        assert!(company_definition.contains("employees: z.array(Employee$Schema)"));
        assert!(company_definition.contains("department_names: z.array(z.string())"));
        assert!(company_definition.contains("headquarters: Address$Schema"));
        assert!(company_definition.contains("settings: CompanySettings$Schema"));

        // Check optional fields in nested structures
        assert!(employee_definition.contains("manager: string | undefined;"));
        assert!(employee_definition.contains("manager: z.string().or(z.undefined())"));

        // Check discriminated union
        assert!(retirement_definition.contains("type: \"option401k\""));
        assert!(retirement_definition.contains("type: \"pension\""));
        assert!(retirement_definition.contains("type: \"roth\""));
        assert!(retirement_definition.contains("employerMatchPercentage: number;"));
        assert!(retirement_definition.contains("yearsOfServiceRequired: number;"));
        assert!(retirement_definition.contains("contributionLimit: number;"));
    }

    // Test serialization consistency
    #[test]
    fn test_serialization_consistency() {
        let project = ProjectJson {
            id: "proj_123".to_string(),
            name: "New Website".to_string(),
            status: ProjectStatusJson::InProgress,
            assigned_employees: vec!["emp_1".to_string(), "emp_2".to_string()],
            deadline: Some("2024-12-31".to_string()),
            budget: Some(50000),
        };

        // Serialize to JSON
        let json_str = serde_json::to_string(&project).unwrap();
        let json_value: Value = serde_json::from_str(&json_str).unwrap();

        // Deserialize back
        let deserialized: ProjectJson = serde_json::from_value(json_value.clone()).unwrap();
        assert_eq!(project, deserialized);

        // Check that the JSON matches expected structure
        assert_eq!(json_value["id"], "proj_123");
        assert_eq!(json_value["name"], "New Website");
        assert_eq!(json_value["status"], "inProgress"); // Should be camelCase
        assert_eq!(json_value["assigned_employees"][0], "emp_1");
        assert_eq!(json_value["deadline"], "2024-12-31");
        assert_eq!(json_value["budget"], 50000);
    }

    // Test edge cases with various field types (simplified)
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct EdgeCasesJson {
        // Different numeric types
        tiny_number: u8,
        small_number: u16,
        medium_number: u32,
        float_number: f32,
        
        // Collections with different types
        strings: Vec<String>,
        numbers: Vec<u32>,
        booleans: Vec<bool>,
        
        // Optional collections
        optional_strings: Option<Vec<String>>,
        optional_numbers: Option<Vec<u32>>,
        
        // Simple maps (only string keys and values supported)
        string_map: HashMap<String, String>,
        
        // Nested optional structures
        nested_optional: Option<ContactInfoJson>,
        nested_array: Vec<ContactInfoJson>,
        optional_nested_array: Option<Vec<ContactInfoJson>>,
    }

    #[test]
    fn test_edge_cases_json_schema() {
        let schema = EdgeCasesJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();

        // Check numeric types
        assert_eq!(properties["tiny_number"]["type"], "integer");
        assert_eq!(properties["small_number"]["type"], "integer");
        assert_eq!(properties["medium_number"]["type"], "integer");
        assert_eq!(properties["float_number"]["type"], "number");

        // Check array types
        assert_eq!(properties["strings"]["type"], "array");
        assert_eq!(properties["strings"]["items"]["type"], "string");
        assert_eq!(properties["numbers"]["type"], "array");
        assert_eq!(properties["numbers"]["items"]["type"], "integer");
        assert_eq!(properties["booleans"]["type"], "array");
        assert_eq!(properties["booleans"]["items"]["type"], "boolean");

        // Check map types
        assert_eq!(properties["string_map"]["type"], "object");
        assert_eq!(properties["string_map"]["additionalProperties"]["type"], "string");

        // Check required vs optional fields
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&Value::String("tiny_number".to_string())));
        assert!(required.contains(&Value::String("strings".to_string())));
        assert!(required.contains(&Value::String("string_map".to_string())));
        assert!(!required.contains(&Value::String("optional_strings".to_string())));
        assert!(!required.contains(&Value::String("nested_optional".to_string())));
        assert!(!required.contains(&Value::String("optional_nested_array".to_string())));
    }

    #[test]
    fn test_edge_cases_ts_definition() {
        let ts_definition = EdgeCasesJson::ts_definition();

        // Check numeric types (all should be 'number' in TypeScript)
        assert!(ts_definition.contains("tiny_number: number;"));
        assert!(ts_definition.contains("small_number: number;"));
        assert!(ts_definition.contains("medium_number: number;"));
        assert!(ts_definition.contains("float_number: number;"));

        // Check array types
        assert!(ts_definition.contains("strings: Array<string>;"));
        assert!(ts_definition.contains("numbers: Array<number>;"));
        assert!(ts_definition.contains("booleans: Array<boolean>;"));

        // Check optional arrays
        assert!(ts_definition.contains("optional_strings: Array<string> | undefined;"));
        assert!(ts_definition.contains("optional_numbers: Array<number> | undefined;"));

        // Check maps (they become Partial<Record<...>> in the generated output)
        assert!(ts_definition.contains("string_map: Partial<Record<string, string>>;"));

        // Check nested types (without Json suffix)
        assert!(ts_definition.contains("nested_optional: ContactInfo | undefined;"));
        assert!(ts_definition.contains("nested_array: Array<ContactInfo>;"));
        assert!(ts_definition.contains("optional_nested_array: Array<ContactInfo> | undefined;"));

        // Check Zod schemas (without Json suffix)
        assert!(ts_definition.contains("tiny_number: z.number().int()"));
        assert!(ts_definition.contains("float_number: z.number()"));
        assert!(ts_definition.contains("optional_strings: z.array(z.string()).or(z.undefined())"));
        assert!(ts_definition.contains("nested_optional: ContactInfo$Schema.or(z.undefined())"));
        assert!(ts_definition.contains("optional_nested_array: z.array(ContactInfo$Schema).or(z.undefined())"));
    }

    // Test discriminated union with complex fields
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    #[serde(tag = "eventType", rename_all = "camelCase")]
    enum ComplexEventJson {
        UserRegistered {
            user_id: String,
            email: String,
            registration_source: String,
            metadata: HashMap<String, String>,
            preferences: Vec<String>,
        },
        PurchaseCompleted {
            user_id: String,
            order_id: String,
            items: Vec<PurchaseItemJson>,
            total_amount: u32,
            payment_method: String,
            shipping_address: Option<AddressJson>,
        },
        SystemMaintenance {
            scheduled_start: String,
            estimated_duration: u32,
            affected_services: Vec<String>,
            notification_sent: bool,
        },
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct PurchaseItemJson {
        product_id: String,
        quantity: u32,
        unit_price: u32,
        discount_applied: Option<u32>,
    }

    #[test]
    fn test_complex_discriminated_union() {
        let schema = ComplexEventJson::json_schema();
        let ts_definition = ComplexEventJson::ts_definition();

        // Check that it's a discriminated union
        assert_eq!(schema["type"], "object");
        assert!(schema.get("oneOf").is_some());
        let one_of = schema["oneOf"].as_array().unwrap();
        assert_eq!(one_of.len(), 3);

        // Check that each variant has the correct discriminator
        for variant in one_of {
            let properties = variant["properties"].as_object().unwrap();
            assert!(properties.contains_key("eventType"));
            assert_eq!(properties["eventType"]["type"], "string");
            assert!(properties["eventType"].get("const").is_some());
        }

        // Check TypeScript definition
        assert!(ts_definition.contains("eventType: \"userRegistered\""));
        assert!(ts_definition.contains("eventType: \"purchaseCompleted\""));
        assert!(ts_definition.contains("eventType: \"systemMaintenance\""));
        
        // Check that field names are converted to camelCase
        assert!(ts_definition.contains("userId: string;"));
        assert!(ts_definition.contains("registrationSource: string;"));
        assert!(ts_definition.contains("orderId: string;"));
        assert!(ts_definition.contains("totalAmount: number;"));
        assert!(ts_definition.contains("paymentMethod: string;"));
        // Address type reference should not have Json suffix
        assert!(ts_definition.contains("shippingAddress: Address | undefined;"));
        assert!(ts_definition.contains("scheduledStart: string;"));
        assert!(ts_definition.contains("estimatedDuration: number;"));
        assert!(ts_definition.contains("affectedServices: Array<string>;"));
        assert!(ts_definition.contains("notificationSent: boolean;"));

        // Check Zod discriminated union
        assert!(ts_definition.contains("z.discriminatedUnion(\"eventType\""));
    }

    // Test with documentation comments
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    /// A user account in the system
    struct DocumentedUserJson {
        /// The unique identifier for the user
        id: String,
        /// The user's full name
        name: String,
        /// The user's email address
        email: String,
        /// Whether the user's account is active
        is_active: bool,
        /// Optional additional metadata
        metadata: Option<HashMap<String, String>>,
    }

    #[test]
    fn test_documented_struct() {
        let schema = DocumentedUserJson::json_schema();
        let ts_definition = DocumentedUserJson::ts_definition();

        // Schema should still be valid
        assert_eq!(schema["type"], "object");
        let properties = schema["properties"].as_object().unwrap();
        assert!(properties.contains_key("id"));
        assert!(properties.contains_key("name"));
        assert!(properties.contains_key("email"));
        assert!(properties.contains_key("is_active"));
        assert!(properties.contains_key("metadata"));

        // TypeScript definition should be generated (without Json suffix)
        assert!(ts_definition.contains("export type DocumentedUser = {"));
        assert!(ts_definition.contains("id: string;"));
        assert!(ts_definition.contains("name: string;"));
        assert!(ts_definition.contains("email: string;"));
        assert!(ts_definition.contains("is_active: boolean;"));
        // HashMap becomes Partial<Record<...>>
        assert!(ts_definition.contains("metadata: Partial<Record<string, string>> | undefined;"));
    }

    // Test validation of generated JSON schemas
    #[test]
    fn test_json_schema_validation() {
        let schemas = vec![
            ("CompanyJson", CompanyJson::json_schema()),
            ("EmployeeJson", EmployeeJson::json_schema()),
            ("ProjectStatusJson", ProjectStatusJson::json_schema()),
            ("RetirementPlanJson", RetirementPlanJson::json_schema()),
            ("EdgeCasesJson", EdgeCasesJson::json_schema()),
            ("ComplexEventJson", ComplexEventJson::json_schema()),
        ];

        for (name, schema) in schemas {
            // All schemas should be objects
            assert!(schema.is_object(), "Schema for {} should be an object", name);
            
            // Should have required fields
            assert!(schema.get("type").is_some(), "Schema for {} should have a type", name);
            
            // Object schemas should have properties
            if schema["type"] == "object" {
                if let Some(one_of) = schema.get("oneOf") {
                    // Discriminated union - check each variant
                    let variants = one_of.as_array().unwrap();
                    for variant in variants {
                        assert!(variant.get("properties").is_some(), 
                            "Discriminated union variant for {} should have properties", name);
                    }
                } else {
                    // Regular object - should have properties
                    assert!(schema.get("properties").is_some(), 
                        "Object schema for {} should have properties", name);
                }
            }
            
            // Should be valid JSON
            let json_str = serde_json::to_string(&schema).unwrap();
            let _: Value = serde_json::from_str(&json_str).unwrap();
        }
    }

    // Test actual serialization and deserialization roundtrip
    #[test]
    fn test_roundtrip_serialization() {
        let contact = ContactInfoJson {
            email: "test@example.com".to_string(),
            phone: Some("123-456-7890".to_string()),
            emergency_contact: Some(EmergencyContactJson {
                name: "John Doe".to_string(),
                relationship: "Brother".to_string(),
                phone: "098-765-4321".to_string(),
            }),
        };

        let employee = EmployeeJson {
            id: "emp_123".to_string(),
            name: "Jane Smith".to_string(),
            position: "Software Engineer".to_string(),
            salary: 75000,
            manager: Some("mgr_456".to_string()),
            skills: vec!["Rust".to_string(), "TypeScript".to_string()],
            contact: contact.clone(),
        };

        // Test serialization
        let json_str = serde_json::to_string(&employee).unwrap();
        let json_value: Value = serde_json::from_str(&json_str).unwrap();

        // Test deserialization
        let deserialized: EmployeeJson = serde_json::from_value(json_value).unwrap();
        assert_eq!(employee, deserialized);

        // Test individual field serialization
        assert_eq!(deserialized.id, "emp_123");
        assert_eq!(deserialized.name, "Jane Smith");
        assert_eq!(deserialized.position, "Software Engineer");
        assert_eq!(deserialized.salary, 75000);
        assert_eq!(deserialized.manager, Some("mgr_456".to_string()));
        assert_eq!(deserialized.skills.len(), 2);
        assert_eq!(deserialized.contact.email, "test@example.com");
        assert_eq!(deserialized.contact.phone, Some("123-456-7890".to_string()));
        assert!(deserialized.contact.emergency_contact.is_some());
    }
} 