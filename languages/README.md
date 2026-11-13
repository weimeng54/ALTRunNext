# ALTRunNext language packs

Copy a UTF-8 JSON pack into the `languages` folder next to `ALTRunNext.exe`. Reopen **Settings → General** to find the new language by its native name.

Start with the [English catalog](../assets/i18n/en.json). Set `locale` to a language tag such as `pt-BR`, set `nativeName` to the name users should see, and keep `formatVersion` at `1`. A file name such as `pt-BR.json` makes the pack easy to identify.

```json
{
  "locale": "pt-BR",
  "nativeName": "Português (Brasil)",
  "formatVersion": 1,
  "strings": {
    "about.title": "Sobre",
    "about.open_releases": "Abrir versões"
  }
}
```

Use the English catalog's resource keys and keep named placeholders such as `{count}`. In menu text, retain the `\t` separating a label from its shortcut. Missing translations fall back to English. Use a unique locale for each pack. For automatic selection, Windows locales such as `ja-JP` can also match the general `ja` pack. The current window layout supports left-to-right languages.

## 中文说明

将 UTF-8 JSON 放进 `ALTRunNext.exe` 同级的 `languages` 文件夹，重新打开「设置 → 常规」即可在「显示语言」里看到语言包的本地名称。

可以复制[英文模板](../assets/i18n/en.json)，填写语言代码 `locale`、本地显示名称 `nativeName`，并保留 `formatVersion: 1`。翻译 `strings` 中的值，保留资源键、`{count}` 等占位符和菜单快捷键前的 `\t`。缺译的条目会显示英文。每个语言包使用独立的语言代码，当前窗口布局适用于从左到右书写的语言。
