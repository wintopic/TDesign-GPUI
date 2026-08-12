//! Compile-time list backing the machine-readable parity manifest.

/// Names of the 71 TDesign React PC component pages represented by this crate.
pub const COMPONENTS: &[&str] = &[
    "Button",
    "Icon",
    "Link",
    "Typography",
    "Divider",
    "Grid",
    "Layout",
    "Space",
    "Affix",
    "Anchor",
    "BackTop",
    "Breadcrumb",
    "Dropdown",
    "Menu",
    "Pagination",
    "Steps",
    "StickyTool",
    "Tabs",
    "Alert",
    "Dialog",
    "Drawer",
    "Guide",
    "Message",
    "Notification",
    "Popconfirm",
    "Popup",
    "AutoComplete",
    "Cascader",
    "Checkbox",
    "ColorPicker",
    "DatePicker",
    "Form",
    "Input",
    "InputAdornment",
    "InputNumber",
    "TagInput",
    "Radio",
    "RangeInput",
    "Select",
    "SelectInput",
    "Slider",
    "Switch",
    "Textarea",
    "Transfer",
    "TimePicker",
    "TreeSelect",
    "Upload",
    "Avatar",
    "Badge",
    "Calendar",
    "Card",
    "Collapse",
    "Comment",
    "Descriptions",
    "Empty",
    "Image",
    "ImageViewer",
    "List",
    "Loading",
    "Progress",
    "QRCode",
    "Skeleton",
    "Statistic",
    "Swiper",
    "Table",
    "Tag",
    "Timeline",
    "Tooltip",
    "Tree",
    "Watermark",
    "Rate",
];

/// Returns the number of component pages represented by the crate.
pub const fn component_count() -> usize {
    COMPONENTS.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_component_pages_are_registered() {
        assert_eq!(component_count(), 71);
    }
}
