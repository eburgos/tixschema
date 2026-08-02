use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tixschema::model_schema;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Address {
    city: String,
    country: String,
    state: String,
    street: String,
    zip_code: String,
}

// Test complex nested structures (fixed to work with actual macro)
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Company {
    // Changed to string keys only - HashMap<String, Department> not supported
    department_names: Vec<String>,
    employees: Vec<Employee>,
    headquarters: Address,
    id: String,
    name: String,
    settings: CompanySettings,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct CompanySettings {
    allow_remote_work: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    health_insurance_provider: Option<String>,
    max_vacation_days: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    retirement_plan: Option<RetirementPlan>,
}

// Test discriminated union with complex fields
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "eventType", rename_all = "camelCase")]
enum ComplexEvent {
    PurchaseCompleted {
        items: Vec<PurchaseItem>,
        order_id: String,
        payment_method: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        shipping_address: Option<Address>,
        total_amount: u32,
        user_id: String,
    },
    SystemMaintenance {
        affected_services: Vec<String>,
        estimated_duration: u32,
        notification_sent: bool,
        scheduled_start: String,
    },
    UserRegistered {
        email: String,
        metadata: HashMap<String, String>,
        preferences: Vec<String>,
        registration_source: String,
        user_id: String,
    },
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ContactInfo {
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    emergency_contact: Option<EmergencyContact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<String>,
}

// Test with documentation comments
/// A user account in the system.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct DocumentedUser {
    /// The user's email address.
    email: String,
    /// The unique identifier for the user.
    id: String,
    /// Whether the user's account is active.
    is_active: bool,
    /// Optional additional metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<HashMap<String, String>>,
    /// The user's full name.
    name: String,
}

// Test edge cases with various field types (simplified)
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct EdgeCases {
    booleans: Vec<bool>,
    float_number: f32,
    medium_number: u32,
    nested_array: Vec<ContactInfo>,
    // Nested optional structures
    #[serde(skip_serializing_if = "Option::is_none")]
    nested_optional: Option<ContactInfo>,
    numbers: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_nested_array: Option<Vec<ContactInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_numbers: Option<Vec<u32>>,
    // Optional collections
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_strings: Option<Vec<String>>,
    small_number: u16,
    // Simple maps (only string keys and values supported)
    string_map: HashMap<String, String>,
    // Collections with different types
    strings: Vec<String>,
    // Different numeric types
    tiny_number: u8,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct EmergencyContact {
    name: String,
    phone: String,
    relationship: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Employee {
    contact: ContactInfo,
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    manager: Option<String>, // Manager ID
    name: String,
    position: String,
    salary: u32,
    skills: Vec<String>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Project {
    assigned_employees: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    budget: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deadline: Option<String>,
    id: String,
    name: String,
    status: ProjectStatus,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
enum ProjectStatus {
    Cancelled,
    Completed,
    InProgress,
    NotStarted,
    OnHold,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct PurchaseItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    discount_applied: Option<u32>,
    product_id: String,
    quantity: u32,
    unit_price: u32,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
enum RetirementPlan {
    Option401k {
        employer_match_percentage: f32,
        vesting_schedule: String,
    },
    Pension {
        monthly_benefit_multiplier: f32,
        years_of_service_required: u32,
    },
    Roth {
        contribution_limit: u32,
        employer_contribution: bool,
    },
}

#[test]
fn test_advanced_types_constructible() {
    let address = Address {
        city: String::new(),
        country: String::new(),
        state: String::new(),
        street: String::new(),
        zip_code: String::new(),
    };
    assert!(address.city.is_empty());
    let settings = CompanySettings {
        allow_remote_work: false,
        health_insurance_provider: None,
        max_vacation_days: 0,
        retirement_plan: None,
    };
    assert!(!settings.allow_remote_work);
    let company = Company {
        department_names: Vec::new(),
        employees: Vec::new(),
        headquarters: address,
        id: String::new(),
        name: String::new(),
        settings,
    };
    assert!(company.id.is_empty());
    let event = ComplexEvent::SystemMaintenance {
        affected_services: Vec::new(),
        estimated_duration: 0,
        notification_sent: false,
        scheduled_start: String::new(),
    };
    assert!(matches!(event, ComplexEvent::SystemMaintenance { .. }));
    let documented = DocumentedUser {
        email: String::new(),
        id: String::new(),
        is_active: false,
        metadata: None,
        name: String::new(),
    };
    assert!(documented.id.is_empty());
    let edge = EdgeCases {
        booleans: Vec::new(),
        float_number: 0.0,
        medium_number: 0,
        nested_array: Vec::new(),
        nested_optional: None,
        numbers: Vec::new(),
        optional_nested_array: None,
        optional_numbers: None,
        optional_strings: None,
        small_number: 0,
        string_map: HashMap::new(),
        strings: Vec::new(),
        tiny_number: 0,
    };
    assert!(edge.booleans.is_empty());
    let item = PurchaseItem {
        discount_applied: None,
        product_id: String::new(),
        quantity: 0,
        unit_price: 0,
    };
    assert!(item.product_id.is_empty());
    let plan = RetirementPlan::Roth {
        contribution_limit: 0,
        employer_contribution: false,
    };
    assert!(matches!(plan, RetirementPlan::Roth { .. }));
}

#[cfg(all(feature = "typescript", feature = "zod"))]
fn assert_ts_contains_fields(ts_definition: &str, assertions: &[(&str, &str)]) {
    for (field, expected_type) in assertions {
        let expected = format!("{field}: {expected_type};");
        assert!(ts_definition.contains(&expected));
    }
}

#[cfg(all(feature = "typescript", feature = "zod"))]
fn assert_zod_contains_fields(zod_schema: &str, assertions: &[(&str, &str)]) {
    for (field, expected_pattern) in assertions {
        let expected = format!("{field}: {expected_pattern}");
        assert!(zod_schema.contains(&expected));
    }
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_complex_nested_json_schema() {
    let company_schema = Company::json_schema();
    let employee_schema = Employee::json_schema();
    let project_schema = Project::json_schema();
    let contact_schema = ContactInfo::json_schema();
    let settings_schema = CompanySettings::json_schema();
    let retirement_schema = RetirementPlan::json_schema();

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
    assert_eq!(
        company_properties["department_names"]["items"]["type"],
        "string"
    );

    // Verify RetirementPlan is a discriminated union
    assert!(retirement_schema.get("oneOf").is_some());
    let one_of = retirement_schema["oneOf"].as_array().unwrap();
    assert_eq!(one_of.len(), 3);
}

#[test]
#[cfg(all(feature = "typescript", feature = "serde", feature = "zod"))]
fn test_complex_nested_ts_definition() {
    let company_definition = Company::ts_definition();
    let employee_definition = Employee::ts_definition();
    let retirement_definition = RetirementPlan::ts_definition();

    // Check that nested types are properly referenced (without Json suffix)
    assert!(company_definition.contains("employees: Array<Employee>;"));
    assert!(company_definition.contains("department_names: Array<string>;"));
    assert!(company_definition.contains("headquarters: Address;"));
    assert!(company_definition.contains("settings: CompanySettings;"));

    // Check optional fields in nested structures
    assert!(employee_definition.contains("manager: string | undefined;"));

    // Check discriminated union
    assert!(retirement_definition.contains("type: \"option401k\""));
    assert!(retirement_definition.contains("type: \"pension\""));
    assert!(retirement_definition.contains("type: \"roth\""));
    assert!(retirement_definition.contains("employerMatchPercentage: number;"));
    assert!(retirement_definition.contains("yearsOfServiceRequired: number;"));
    assert!(retirement_definition.contains("contributionLimit: number;"));

    // Check Zod schema references (without Json suffix) - now in separate method
    let company_zod_schema = Company::zod_schema();
    let employee_zod_schema = Employee::zod_schema();

    assert!(company_zod_schema.contains("employees: z.array(Employee$Schema)"));
    assert!(company_zod_schema.contains("department_names: z.array(z.string())"));
    assert!(company_zod_schema.contains("headquarters: Address$Schema"));
    assert!(company_zod_schema.contains("settings: CompanySettings$Schema"));
    assert!(employee_zod_schema.contains("manager: z.union([z.string(), z.undefined()])"));
}

// Test serialization consistency
#[test]
fn test_serialization_consistency() {
    let project = Project {
        id: "proj_123".to_owned(),
        name: "New Website".to_owned(),
        status: ProjectStatus::InProgress,
        assigned_employees: vec!["emp_1".to_owned(), "emp_2".to_owned()],
        deadline: Some("2024-12-31".to_owned()),
        budget: Some(50000),
    };

    // Serialize to JSON
    let json_str = serde_json::to_string(&project).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();

    // Deserialize back
    let deserialized: Project = serde_json::from_value(json_value.clone()).unwrap();
    assert_eq!(project, deserialized);

    // Check that the JSON matches expected structure
    assert_eq!(json_value["id"], "proj_123");
    assert_eq!(json_value["name"], "New Website");
    assert_eq!(json_value["status"], "inProgress"); // Should be camelCase
    assert_eq!(json_value["assigned_employees"][0], "emp_1");
    assert_eq!(json_value["deadline"], "2024-12-31");
    assert_eq!(json_value["budget"], 50000_i64);
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_edge_cases_json_schema() {
    let schema = EdgeCases::json_schema();
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
    assert_eq!(
        properties["string_map"]["additionalProperties"]["type"],
        "string"
    );

    // Check required vs optional fields
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&Value::String("tiny_number".to_owned())));
    assert!(required.contains(&Value::String("strings".to_owned())));
    assert!(required.contains(&Value::String("string_map".to_owned())));
    assert!(!required.contains(&Value::String("optional_strings".to_owned())));
    assert!(!required.contains(&Value::String("nested_optional".to_owned())));
    assert!(!required.contains(&Value::String("optional_nested_array".to_owned())));
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_edge_cases_ts_definition() {
    let ts_definition = EdgeCases::ts_definition();

    // Check numeric types, arrays, optional arrays, maps, and nested types
    assert_ts_contains_fields(
        &ts_definition,
        &[
            ("tiny_number", "number"),
            ("small_number", "number"),
            ("medium_number", "number"),
            ("float_number", "number"),
            ("strings", "Array<string>"),
            ("numbers", "Array<number>"),
            ("booleans", "Array<boolean>"),
            ("optional_strings", "Array<string> | undefined"),
            ("optional_numbers", "Array<number> | undefined"),
            ("string_map", "Partial<Record<string, string>>"),
            ("nested_optional", "ContactInfo | undefined"),
            ("nested_array", "Array<ContactInfo>"),
            ("optional_nested_array", "Array<ContactInfo> | undefined"),
        ],
    );

    // Check Zod schemas
    let zod_schema = EdgeCases::zod_schema();
    assert_zod_contains_fields(
        &zod_schema,
        &[
            ("tiny_number", "z.number().int()"),
            ("small_number", "z.number().int()"),
            ("medium_number", "z.number().int()"),
            ("float_number", "z.number()"),
            ("strings", "z.array(z.string())"),
            ("numbers", "z.array(z.number().int())"),
            ("booleans", "z.array(z.boolean())"),
            (
                "optional_strings",
                "z.union([z.array(z.string()), z.undefined()])",
            ),
            (
                "optional_numbers",
                "z.union([z.array(z.number().int()), z.undefined()])",
            ),
            ("string_map", "z.record(z.string(), z.string())"),
            (
                "nested_optional",
                "z.union([ContactInfo$Schema, z.undefined()])",
            ),
            ("nested_array", "z.array(ContactInfo$Schema)"),
            (
                "optional_nested_array",
                "z.union([z.array(ContactInfo$Schema), z.undefined()])",
            ),
        ],
    );
}

#[test]
#[cfg(all(
    feature = "jsonschema",
    feature = "typescript",
    feature = "serde",
    feature = "zod"
))]
fn test_complex_discriminated_union() {
    let schema = ComplexEvent::json_schema();
    let ts_definition = ComplexEvent::ts_definition();

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

    // Check Zod discriminated union - now in separate method
    let zod_schema = ComplexEvent::zod_schema();
    assert!(zod_schema.contains("z.discriminatedUnion(\"eventType\""));
}

#[test]
#[cfg(all(feature = "jsonschema", feature = "typescript"))]
fn test_documented_struct() {
    let schema = DocumentedUser::json_schema();
    let ts_definition = DocumentedUser::ts_definition();

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
#[cfg(feature = "jsonschema")]
fn test_json_schema_validation() {
    let schemas = vec![
        ("Company", Company::json_schema()),
        ("Employee", Employee::json_schema()),
        ("ProjectStatus", ProjectStatus::json_schema()),
        ("RetirementPlan", RetirementPlan::json_schema()),
        ("EdgeCases", EdgeCases::json_schema()),
        ("ComplexEvent", ComplexEvent::json_schema()),
    ];

    for (name, schema) in schemas {
        // All schemas should be objects
        assert!(schema.is_object(), "Schema for {name} should be an object");

        // Should have required fields
        assert!(
            schema.get("type").is_some(),
            "Schema for {name} should have a type"
        );

        // Object schemas should have properties
        if schema["type"] == "object" {
            if let Some(one_of) = schema.get("oneOf") {
                // Discriminated union - check each variant
                let variants = one_of.as_array().unwrap();
                for variant in variants {
                    assert!(
                        variant.get("properties").is_some(),
                        "Discriminated union variant for {name} should have properties"
                    );
                }
            } else {
                // Regular object - should have properties
                assert!(
                    schema.get("properties").is_some(),
                    "Object schema for {name} should have properties"
                );
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
    let contact = ContactInfo {
        email: "test@example.com".to_owned(),
        phone: Some("123-456-7890".to_owned()),
        emergency_contact: Some(EmergencyContact {
            name: "John Doe".to_owned(),
            relationship: "Brother".to_owned(),
            phone: "098-765-4321".to_owned(),
        }),
    };

    let employee = Employee {
        id: "emp_123".to_owned(),
        name: "Jane Smith".to_owned(),
        position: "Software Engineer".to_owned(),
        salary: 75000,
        manager: Some("mgr_456".to_owned()),
        skills: vec!["Rust".to_owned(), "TypeScript".to_owned()],
        contact,
    };

    // Test serialization
    let json_str = serde_json::to_string(&employee).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();

    // Test deserialization
    let deserialized: Employee = serde_json::from_value(json_value).unwrap();
    assert_eq!(employee, deserialized);

    // Test individual field serialization
    assert_eq!(deserialized.id, "emp_123");
    assert_eq!(deserialized.name, "Jane Smith");
    assert_eq!(deserialized.position, "Software Engineer");
    assert_eq!(deserialized.salary, 75000);
    assert_eq!(deserialized.manager, Some("mgr_456".to_owned()));
    assert_eq!(deserialized.skills.len(), 2);
    assert_eq!(deserialized.contact.email, "test@example.com");
    assert_eq!(deserialized.contact.phone, Some("123-456-7890".to_owned()));
    assert!(deserialized.contact.emergency_contact.is_some());
}
