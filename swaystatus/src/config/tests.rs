use super::*;
use crate::plugin::*;
use crate::test_plugin::TestPlugin; 
use crate::plugin_database::test_helper::*;

//to test the internals of the custom deserialize implementation, we need to check how it 
//interacts with the deserializer and the map access. To the mock-mobile!
//However, mockall can't be used to mock a Deserializer, becauseit doesn't support lifetimes
//on return types. That's why we mock by hand :-(
#[derive(Debug)]
struct MockError(String);
impl std::fmt::Display for MockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for MockError {}
impl serde::de::Error for MockError {
    fn custom<T: fmt::Display>(msg: T) -> MockError {
        MockError(msg.to_string())
    }
}

/**
 * Deserializer that only exists to check if ConfigVisitor calls the correct function.
 * Returns Err(MockError(String::from("Correct"))); on _success_, asserts on failure.
 * The reason for not returning Ok is simple: supporting it would bloat this mock beyond
 * reason, as we would need to supply data to the erased_serde visitor...
 */
struct MockDeserializerForPluginConfigDeserializeSeed;
impl<'de> serde::Deserializer<'de> for MockDeserializerForPluginConfigDeserializeSeed {
    type Error = MockError;
    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, MockError>
    where  V: de::Visitor<'de>, {
        assert!(false);
        return Err(MockError(String::from("Unexpected Type")));
    }
    fn deserialize_struct<V>(self, name: &'static str, fields: &'static [&'static str], _visitor: V) -> Result<V::Value, MockError>
    where V: de::Visitor<'de>, {
        //The line below would be awesome, but Visitor doesn't need static lifetime.
        //Instead we now just assert on the struct and field names.
        //assert_eq!(std::any::TypeId::of::<test_plugin::TestConfig>(), std::any::TypeId::of::<V::Value>());
        assert_eq!(name, "TestConfig");
        assert_eq!(fields[0], "lines");
        assert_eq!(fields[1], "skull");
        //without actual data to deserialize, it's hard to fake an OK...
        //For testing we just return an error here.
        return Err(MockError(String::from("Correct")));
    }
    serde::forward_to_deserialize_any! {
        enum bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string seq
        bytes byte_buf map unit newtype_struct
        ignored_any unit_struct tuple_struct tuple option identifier
    }
}

#[test]
fn plugin_config_deserialize_seed_calls_correct_plugin() {
    let p = get_plugin_database_with_test_plugin(); 

    let plugin_name = String::from(TestPlugin.get_name());
    let v = custom_deserializers::PluginConfigDeserializeSeed(&p, &plugin_name);
    let result = v.deserialize(MockDeserializerForPluginConfigDeserializeSeed);
    assert!(result.is_err());
    assert_eq!(result.err().unwrap().0, "Correct");
}

#[test]
fn plugin_config_deserialize_seed_correct_plugin_not_found_error()
{
    let p = get_plugin_database_empty();
    let plugin_name = String::from(TestPlugin.get_name());
    let v = custom_deserializers::PluginConfigDeserializeSeed(&p, &plugin_name);
    let result = v.deserialize(MockDeserializerForPluginConfigDeserializeSeed);
    assert!(result.is_err());
    assert_eq!(result.err().unwrap().0, "Plugin not found");
}

#[test]
fn custom_deserialize_optional_field_settings()
{
    let p = get_plugin_database_with_test_plugin();
    let test_config = String::from(
    "[[Element]]\nPlugin = \"TestPlugin\"\n\n[Element.Config]\nlines = 2\nskull = \"skully\"\n");
    let deserialized = SwaystatusConfig::deserialize(&test_config, &p).unwrap();
    let serialized = toml::to_string(&deserialized).unwrap();
    //println!("{}", serialized);
    assert_eq!(test_config, serialized);
}

#[test]
fn custom_deserialize_optional_field_elements()
{
    let p = get_plugin_database_with_test_plugin();
    let test_config = String::from(
    "[Settings]\nseparator = \"Kisses!\"\n"
    );
    let deserialized = SwaystatusConfig::deserialize(&test_config, &p).unwrap();
    let serialized = toml::to_string(&deserialized).unwrap();
    //println!("{}", serialized);
    assert_eq!(test_config, serialized);
}


#[test]
fn custom_deserialize_multiple_plugins()
{
    let p = get_plugin_database_with_test_plugin();
    let test_config = String::from(
    "[[Element]]\nPlugin = \"TestPlugin\"\n\n[Element.Config]\nlines = 2\nskull = \"bones\"\n\n[[Element]]\nPlugin = \"TestPlugin\"\n\n[Element.Config]\nlines = 5\nskull = \"pirate\"\n");
    let deserialized = SwaystatusConfig::deserialize(&test_config, &p).unwrap();
    let serialized = toml::to_string(&deserialized).unwrap();
    //println!("{}", serialized);
    assert_eq!(test_config, serialized);
}

//this is strictly speaking not a unit test, and more a test of how the custom deserialization
//integrates with serde. But it's trivial to do, and tests an important aspect of the code.
#[test]
fn self_consistency(){
    let p = get_plugin_database_with_test_plugin();
    let def = SwaystatusConfig::create_default(&p);
    let serialized = def.serialize().unwrap();
    let deserialized = SwaystatusConfig::deserialize(&serialized, &p).unwrap();
    let serialized2 = toml::to_string(&deserialized).unwrap();
    //println!("{}", serialized2);
    assert_eq!(serialized, serialized2);
}

