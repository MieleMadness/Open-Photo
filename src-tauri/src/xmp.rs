//! Namespace-aware keyword updates that leave all existing XML bytes intact.
use roxmltree::{Document, Node};

const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const DC: &str = "http://purl.org/dc/elements/1.1/";
const XMP: &str = "http://ns.adobe.com/xap/1.0/";

fn keyword_bag<'a>(doc: &'a Document<'a>) -> Option<Node<'a, 'a>> {
    let mut subjects = doc.descendants().filter(|node| {
        node.has_tag_name((DC, "subject"))
            && node.parent().is_some_and(|description| {
                description.has_tag_name((RDF, "Description"))
                    && (description.attribute((XMP, "CreatorTool")) == Some("Open-Photo")
                        || description.children().any(|child| {
                            child.has_tag_name((XMP, "CreatorTool"))
                                && child.text() == Some("Open-Photo")
                        }))
            })
    });
    let subject = subjects.next()?;
    if subjects.next().is_some() {
        return None;
    }
    let mut children = subject.children().filter(Node::is_element);
    let bag = children.next()?;
    if !bag.has_tag_name((RDF, "Bag")) || children.next().is_some() {
        return None;
    }
    Some(bag)
}

pub(crate) fn tags(xml: &str) -> Option<Vec<String>> {
    let doc = Document::parse(xml).ok()?;
    let bag = keyword_bag(&doc)?;
    let mut tags = Vec::new();
    for item in bag.children().filter(Node::is_element) {
        // Do not reinterpret structured RDF values as simple keywords.
        if !item.has_tag_name((RDF, "li")) || item.children().any(|node| node.is_element()) {
            return None;
        }
        let value: String = item
            .children()
            .filter(Node::is_text)
            .filter_map(|node| node.text())
            .collect();
        if value.is_empty() || value.len() > 128 {
            return None;
        }
        tags.push(value);
    }
    (tags.len() <= 200).then_some(tags)
}

pub(crate) fn append_tags(xml: &str, additions: &[String]) -> Option<String> {
    let mut existing = tags(xml)?;
    let mut inserted = String::new();
    for tag in additions {
        if !existing.iter().any(|old| old.eq_ignore_ascii_case(tag)) {
            inserted.push_str(&format!(
                "<rdf:li xmlns:rdf=\"{RDF}\">{}</rdf:li>",
                crate::xml_escape(tag)
            ));
            existing.push(tag.clone());
        }
    }
    if existing.len() > 200 {
        return None;
    }
    if inserted.is_empty() {
        return Some(xml.to_owned());
    }
    let doc = Document::parse(xml).ok()?;
    let bag = keyword_bag(&doc)?;
    let range = bag.range();
    let original = &xml[range.clone()];
    let replacement = if let Some(open) = original.strip_suffix("/>") {
        let name_end = open
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(open.len());
        let name = &open[1..name_end];
        format!("{open}>{inserted}</{name}>")
    } else {
        let close = original.rfind("</")?;
        format!("{}{inserted}{}", &original[..close], &original[close..])
    };
    let mut result = xml.to_owned();
    result.replace_range(range, &replacement);
    // Validate the patched XML before it can reach a write path.
    tags(&result)?;
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_unknown_metadata_and_namespace_aliases() {
        let xml = format!(
            r#"<r:RDF xmlns:r="{RDF}" xmlns:d="{DC}" xmlns:m="{XMP}" xmlns:u="urn:test"><r:Description m:CreatorTool="Open-Photo" m:Rating="5"><d:description>Caption &amp; text</d:description><u:regions><u:face id="42"/></u:regions><d:subject>
<r:Bag><r:li>Family &#x26; Friends</r:li><!--keep--></r:Bag>
</d:subject></r:Description></r:RDF>"#
        );
        assert_eq!(tags(&xml).unwrap(), vec!["Family & Friends"]);
        let result = append_tags(&xml, &["Trips/Beach".into()]).unwrap();
        let insertion = format!("<rdf:li xmlns:rdf=\"{RDF}\">Trips/Beach</rdf:li>");
        assert_eq!(result.replace(&insertion, ""), xml);
        assert_eq!(tags(&result).unwrap().len(), 2);
    }

    #[test]
    fn supports_empty_bags_and_is_idempotent() {
        let xml = crate::render_openphoto_xmp(&[]).replace("<rdf:Bag></rdf:Bag>", "<rdf:Bag />");
        let result = append_tags(&xml, &["A & B".into()]).unwrap();
        assert_eq!(tags(&result).unwrap(), vec!["A & B"]);
        assert_eq!(append_tags(&result, &["a & b".into()]).unwrap(), result);
    }

    #[test]
    fn rejects_foreign_malformed_and_ambiguous_documents() {
        assert!(tags("<broken>").is_none());
        let xml = crate::render_openphoto_xmp(&["one".into()]);
        assert!(tags(&xml.replace("Open-Photo", "Other App")).is_none());
        assert!(tags(&xml.replace(
            "</rdf:Description>",
            "<dc:subject><rdf:Bag/></dc:subject></rdf:Description>"
        ))
        .is_none());
        assert!(tags(&xml.replace(
            "<rdf:li>one</rdf:li>",
            "<rdf:li><rdf:value>one</rdf:value></rdf:li>"
        ))
        .is_none());
    }
}
