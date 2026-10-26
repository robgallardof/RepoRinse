# text.invisible-unicode

Reports bidi control characters, zero-width spaces, word joiners and embedded BOM characters.
Modes: `off`, `warn` (default), `block`. No automatic fix: these characters can be intentional.
The decoder preserves a leading UTF-8 BOM as encoding metadata; it is not a finding.

Findings include only the Unicode code point and coordinates, never source text.
Accents, non-Latin scripts, non-breaking spaces, variation selectors and emoji joiners are not flagged.
Use per-rule exclusions for deliberate bidi or invisible-character test fixtures and localized content.

## Español

Detecta controles bidireccionales, espacios de ancho cero, separadores invisibles y BOM dentro del texto.
Modos: `off`, `warn` (predeterminado), `block`. No ofrece correcciones automáticas porque pueden ser intencionales.
El BOM UTF-8 inicial se conserva como metadato y no genera hallazgos.
Los mensajes muestran únicamente el código Unicode y su posición, nunca el contenido de la línea.
No marca acentos, alfabetos no latinos, espacios inseparables ni uniones de emoji.
Usa exclusiones por regla para archivos que contengan estos caracteres deliberadamente.
