# tdesign-gpui-assets

Generated TDesign icon names, sanitized embedded SVG assets, and a composable
GPUI `AssetSource` for the unofficial community project
[TDesign GPUI](https://github.com/wintopic/TDesign-GPUI).

The default `full-icons` feature embeds all 2,354 icons. Disable default
features to embed the documented core icon subset. `IconName::ALL` still
describes the complete upstream catalog; call `TDesignAssetSource::contains`
before rendering a dynamically selected icon in minimal builds. Missing assets
render as an explicit placeholder in `tdesign-gpui::Icon`, never as silent
empty space. TDesign assets retain their upstream MIT attribution in
`LICENSE-TDESIGN`.
