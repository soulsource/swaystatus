use super::*;

#[derive(Deserialize)]
#[serde(field_identifier)]
enum SwaystatusConfigField { 
    #[serde(alias = "settings")]
    Settings, 
    #[serde(alias = "element", alias = "elements", alias = "Elements")]
    Element 
}
struct SwaystatusConfigVisitor<'a>(&'a PluginDatabase<'a>) ;
impl<'de, 'a> Visitor<'de> for SwaystatusConfigVisitor<'a> {
    type Value = SwaystatusConfig<'a>;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("struct SwaystatusConfig")
    }
    fn visit_map<V>(self, mut map: V) -> Result<SwaystatusConfig<'a>, V::Error>
    where V: MapAccess<'de>, {
        let mut sett = None;
        let mut elem = None;
        while let Some(key) = map.next_key()? {
            match key {
                SwaystatusConfigField::Settings => {
                    if sett.is_some() {
                        return Err(de::Error::duplicate_field("Settings"));
                    }
                    sett = Some(map.next_value()?);
                }
                SwaystatusConfigField::Element => {
                    if elem.is_some() {
                        return Err(de::Error::duplicate_field("Elements"));
                    }
                    elem = map.next_value_seed(ElementsOptionDeserialize(self.0))?;
                }
            }
        }
        Ok(SwaystatusConfig {
            settings : sett,
            elements : elem
        })
    }
}
pub struct SwaystatusConfigDeserializeSeed<'a>(pub &'a PluginDatabase<'a>);
impl<'de, 'a> DeserializeSeed<'de> for SwaystatusConfigDeserializeSeed<'a> {
    type Value = SwaystatusConfig<'a>;
    fn deserialize<D>(self, deserializer : D) -> Result<Self::Value, D::Error>
    where D: Deserializer<'de> {
        const FIELDS: &[&str] = &["settings", "elements"];
        deserializer.deserialize_struct("SwaystatusConfig", FIELDS, SwaystatusConfigVisitor(self.0))
    }
}

struct ElementsOptionVisitor<'a>(&'a PluginDatabase<'a>);
impl<'de, 'a> Visitor<'de> for ElementsOptionVisitor<'a> {
    type Value = Option<Vec<SwaystatusPluginConfig<'a>>>;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Option<Vec<struct SwaystatusPluginConfig>>")
    }
    fn visit_none<E>(self) -> Result<Self::Value, E> 
    where E: Error, {
        Ok(None)
    }
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where D: Deserializer<'de>, {
        Ok(Some(deserializer.deserialize_seq(ElementsVisitor(self.0))?))
    }
}

struct ElementsOptionDeserialize<'a>(&'a PluginDatabase<'a>);
impl<'de, 'a> DeserializeSeed<'de> for ElementsOptionDeserialize<'a> {
    type Value = Option<Vec<SwaystatusPluginConfig<'a>>>;
    fn deserialize<D>(self, deserializer : D) -> Result<Self::Value, D::Error>
    where D: Deserializer<'de> {
        deserializer.deserialize_option(ElementsOptionVisitor(self.0))
    }
}

struct ElementsVisitor<'a>(&'a PluginDatabase<'a>);
impl<'de, 'a> Visitor<'de> for ElementsVisitor<'a> {
    type Value = Vec<SwaystatusPluginConfig<'a>>;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Vec<struct SwaystatusPluginConfig>")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: SeqAccess<'de>, {
        let mut res : Self::Value = Vec::new();
        while let Some(next_elem) = seq.next_element_seed(SwaystatusPluginConfigSeed(self.0))? {
            res.push(next_elem);
        }
        Ok(res)
    }
}

/**
 * Visitor for config deserialization. Forwards the deserialization request to the
 * respective plugin.
 */
pub struct PluginConfigDeserializeSeed<'a, 'b>(pub &'b PluginDatabase<'b>, pub &'a String);
impl<'de, 'a, 'b> DeserializeSeed<'de> for PluginConfigDeserializeSeed<'a, 'b> {
    type Value = Box<dyn plugin::SwayStatusModuleInstance + 'b>;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> 
    where D: Deserializer<'de> {
        let optplugin = &self.0.get_plugin(self.1);
        let plugin = match &optplugin {
            Some(x) => x,
            None => return Err(de::Error::custom("Plugin not found"))
        };
        let mut erased_deserializer = erased_serde::Deserializer::erase(deserializer);
        plugin.deserialize_config(&mut erased_deserializer).map_err(Error::custom)
    }
}
#[derive(Deserialize)]
#[serde(field_identifier)]
#[allow(non_camel_case_types)]
enum PluginConfigField { Plugin, Config, plugin, config }
struct PluginConfigVisitor<'a>(&'a PluginDatabase<'a>);
impl<'de, 'a> Visitor<'de> for PluginConfigVisitor<'a> {
    type Value = SwaystatusPluginConfig<'a>;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("struct SwaystatusPluginConfig")
    }

    fn visit_map<V>(self, mut map: V) -> Result<SwaystatusPluginConfig<'a>, V::Error>
    where V: MapAccess<'de>, {
        let mut plug = None;
        let mut conf = None;
        while let Some(key) = map.next_key()? {
            match key {
                PluginConfigField::Plugin | PluginConfigField::plugin => {
                    if plug.is_some() {
                        return Err(de::Error::duplicate_field("Plugin"));
                    }
                    plug = Some(map.next_value()?);
                }
                PluginConfigField::Config | PluginConfigField::config => {
                    if conf.is_some(){
                        return Err(de::Error::duplicate_field("Config"));
                    }
                    if let Some(plugin) = plug.as_ref() {
                        conf = Some(map.next_value_seed(PluginConfigDeserializeSeed(self.0, plugin))?);
                    }
                    else {
                        return Err(de::Error::missing_field("Plugin"));
                    }
                }
            }
        }
        Ok(SwaystatusPluginConfig{
            plugin: plug.ok_or_else(|| de::Error::missing_field("Plugin"))?,
            config : conf.ok_or_else(|| de::Error::missing_field("Config"))?
        })
    }
}
struct SwaystatusPluginConfigSeed<'a>(&'a PluginDatabase<'a>);
impl<'de, 'a> DeserializeSeed<'de> for SwaystatusPluginConfigSeed<'a> {
    type Value = SwaystatusPluginConfig<'a>;
    fn deserialize<D>(self, deserializer: D) -> Result<SwaystatusPluginConfig<'a>, D::Error>
    where D: Deserializer<'de> {
        const FIELDS: &[&str] = &["plugin", "config"];
        deserializer.deserialize_struct("SwaystatusPluginConfig", FIELDS, PluginConfigVisitor(self.0))
    }
}

