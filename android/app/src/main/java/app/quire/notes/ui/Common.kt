package app.quire.notes.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.max
import kotlin.math.min

/** Application UI icons from Phosphor Icons (https://phosphoricons.com, MIT license), on a 256×256 viewBox. */
object QIcons {
    // sidebar-simple (regular)
    const val SIDEBAR_LEFT = "M216,40H40A16,16,0,0,0,24,56V200a16,16,0,0,0,16,16H216a16,16,0,0,0,16-16V56A16,16,0,0,0,216,40ZM40,56H80V200H40ZM216,200H96V56H216V200Z"
    // sidebar-simple (regular, mirrored horizontally)
    const val SIDEBAR_RIGHT = "M216,40H40A16,16,0,0,0,24,56V200a16,16,0,0,0,16,16H216a16,16,0,0,0,16-16V56A16,16,0,0,0,216,40ZM40,56H160V200H40ZM216,200H176V56H216V200Z"
    // plus (regular)
    const val PLUS = "M224,128a8,8,0,0,1-8,8H136v80a8,8,0,0,1-16,0V136H40a8,8,0,0,1,0-16h80V40a8,8,0,0,1,16,0v80h80A8,8,0,0,1,224,128Z"
    // x (regular)
    const val CLOSE = "M205.66,194.34a8,8,0,0,1-11.32,11.32L128,139.31,61.66,205.66a8,8,0,0,1-11.32-11.32L116.69,128,50.34,61.66A8,8,0,0,1,61.66,50.34L128,116.69l66.34-66.35a8,8,0,0,1,11.32,11.32L139.31,128Z"
    // caret-down (bold)
    const val CHEVRON_DOWN = "M216.49,104.49l-80,80a12,12,0,0,1-17,0l-80-80a12,12,0,0,1,17-17L128,159l71.51-71.52a12,12,0,0,1,17,17Z"
    // caret-right (bold)
    const val CHEVRON_RIGHT = "M184.49,136.49l-80,80a12,12,0,0,1-17-17L159,128,87.51,56.49a12,12,0,1,1,17-17l80,80A12,12,0,0,1,184.49,136.49Z"
    // folder-simple (regular)
    const val FOLDER_CLOSED = "M216,72H130.67L102.93,51.2a16.12,16.12,0,0,0-9.6-3.2H40A16,16,0,0,0,24,64V200a16,16,0,0,0,16,16H216.89A15.13,15.13,0,0,0,232,200.89V88A16,16,0,0,0,216,72Zm0,128H40V64H93.33L123.2,86.4A8,8,0,0,0,128,88h88Z"
    // folder-open (regular)
    const val FOLDER_OPEN = "M245,110.64A16,16,0,0,0,232,104H216V88a16,16,0,0,0-16-16H130.67L102.94,51.2a16.14,16.14,0,0,0-9.6-3.2H40A16,16,0,0,0,24,64V208h0a8,8,0,0,0,8,8H211.1a8,8,0,0,0,7.59-5.47l28.49-85.47A16.05,16.05,0,0,0,245,110.64ZM93.34,64,123.2,86.4A8,8,0,0,0,128,88h72v16H69.77a16,16,0,0,0-15.18,10.94L40,158.7V64Zm112,136H43.1l26.67-80H232Z"
    // magnifying-glass (regular)
    const val SEARCH = "M229.66,218.34l-50.07-50.06a88.11,88.11,0,1,0-11.31,11.31l50.06,50.07a8,8,0,0,0,11.32-11.32ZM40,112a72,72,0,1,1,72,72A72.08,72.08,0,0,1,40,112Z"
    // check (bold)
    const val CHECK = "M232.49,80.49l-128,128a12,12,0,0,1-17,0l-56-56a12,12,0,1,1,17-17L96,183,215.51,63.51a12,12,0,0,1,17,17Z"
    // eye (regular)
    const val EYE = "M247.31,124.76c-.35-.79-8.82-19.58-27.65-38.41C194.57,61.26,162.88,48,128,48S61.43,61.26,36.34,86.35C17.51,105.18,9,124,8.69,124.76a8,8,0,0,0,0,6.5c.35.79,8.82,19.57,27.65,38.4C61.43,194.74,93.12,208,128,208s66.57-13.26,91.66-38.34c18.83-18.83,27.3-37.61,27.65-38.4A8,8,0,0,0,247.31,124.76ZM128,192c-30.78,0-57.67-11.19-79.93-33.25A133.47,133.47,0,0,1,25,128,133.33,133.33,0,0,1,48.07,97.25C70.33,75.19,97.22,64,128,64s57.67,11.19,79.93,33.25A133.46,133.46,0,0,1,231.05,128C223.84,141.46,192.43,192,128,192Zm0-112a48,48,0,1,0,48,48A48.05,48.05,0,0,0,128,80Zm0,80a32,32,0,1,1,32-32A32,32,0,0,1,128,160Z"
    // code (regular)
    const val CODE = "M69.12,94.15,28.5,128l40.62,33.85a8,8,0,1,1-10.24,12.29l-48-40a8,8,0,0,1,0-12.29l48-40a8,8,0,0,1,10.24,12.3Zm176,27.7-48-40a8,8,0,1,0-10.24,12.3L227.5,128l-40.62,33.85a8,8,0,1,0,10.24,12.29l48-40a8,8,0,0,0,0-12.29ZM162.73,32.48a8,8,0,0,0-10.25,4.79l-64,176a8,8,0,0,0,4.79,10.26A8.14,8.14,0,0,0,96,224a8,8,0,0,0,7.52-5.27l64-176A8,8,0,0,0,162.73,32.48Z"
    // text-b (regular)
    const val BOLD = "M178.48,115.7A44,44,0,0,0,148,40H80a8,8,0,0,0-8,8V200a8,8,0,0,0,8,8h80a48,48,0,0,0,18.48-92.3ZM88,56h60a28,28,0,0,1,0,56H88Zm72,136H88V128h72a32,32,0,0,1,0,64Z"
    // text-italic (regular)
    const val ITALIC = "M200,56a8,8,0,0,1-8,8H157.77L115.1,192H144a8,8,0,0,1,0,16H64a8,8,0,0,1,0-16H98.23L140.9,64H112a8,8,0,0,1,0-16h80A8,8,0,0,1,200,56Z"
    // text-strikethrough (regular)
    const val STRIKE = "M224,128a8,8,0,0,1-8,8H175.93c9.19,7.11,16.07,17.2,16.07,32,0,13.34-7,25.7-19.75,34.79C160.33,211.31,144.61,216,128,216s-32.33-4.69-44.25-13.21C71,193.7,64,181.34,64,168a8,8,0,0,1,16,0c0,17.35,22,32,48,32s48-14.65,48-32c0-14.85-10.54-23.58-38.77-32H40a8,8,0,0,1,0-16H216A8,8,0,0,1,224,128ZM76.33,104a8,8,0,0,0,7.61-10.49A17.3,17.3,0,0,1,83.11,88c0-18.24,19.3-32,44.89-32,18.84,0,34.16,7.42,41,19.85a8,8,0,0,0,14-7.7C173.33,50.52,152.77,40,128,40,93.29,40,67.11,60.63,67.11,88a33.73,33.73,0,0,0,1.62,10.49A8,8,0,0,0,76.33,104Z"
    // text-h (regular)
    const val HEADING = "M208,56V200a8,8,0,0,1-16,0V136H64v64a8,8,0,0,1-16,0V56a8,8,0,0,1,16,0v64H192V56a8,8,0,0,1,16,0Z"
    // link-simple (regular)
    const val LINK = "M165.66,90.34a8,8,0,0,1,0,11.32l-64,64a8,8,0,0,1-11.32-11.32l64-64A8,8,0,0,1,165.66,90.34ZM215.6,40.4a56,56,0,0,0-79.2,0L106.34,70.45a8,8,0,0,0,11.32,11.32l30.06-30a40,40,0,0,1,56.57,56.56l-30.07,30.06a8,8,0,0,0,11.31,11.32L215.6,119.6a56,56,0,0,0,0-79.2ZM138.34,174.22l-30.06,30.06a40,40,0,1,1-56.56-56.57l30.05-30.05a8,8,0,0,0-11.32-11.32L40.4,136.4a56,56,0,0,0,79.2,79.2l30.06-30.07a8,8,0,0,0-11.32-11.31Z"
    // brackets-square (regular)
    const val WIKI = "M48,48V208H80a8,8,0,0,1,0,16H40a8,8,0,0,1-8-8V40a8,8,0,0,1,8-8H80a8,8,0,0,1,0,16ZM216,32H176a8,8,0,0,0,0,16h32V208H176a8,8,0,0,0,0,16h40a8,8,0,0,0,8-8V40A8,8,0,0,0,216,32Z"
    // list-bullets (regular)
    const val LIST = "M80,64a8,8,0,0,1,8-8H216a8,8,0,0,1,0,16H88A8,8,0,0,1,80,64Zm136,56H88a8,8,0,0,0,0,16H216a8,8,0,0,0,0-16Zm0,64H88a8,8,0,0,0,0,16H216a8,8,0,0,0,0-16ZM44,52A12,12,0,1,0,56,64,12,12,0,0,0,44,52Zm0,64a12,12,0,1,0,12,12A12,12,0,0,0,44,116Zm0,64a12,12,0,1,0,12,12A12,12,0,0,0,44,180Z"
    // list-numbers (regular)
    const val NUMBERS = "M224,128a8,8,0,0,1-8,8H104a8,8,0,0,1,0-16H216A8,8,0,0,1,224,128ZM104,72H216a8,8,0,0,0,0-16H104a8,8,0,0,0,0,16ZM216,184H104a8,8,0,0,0,0,16H216a8,8,0,0,0,0-16ZM43.58,55.16,48,52.94V104a8,8,0,0,0,16,0V40a8,8,0,0,0-11.58-7.16l-16,8a8,8,0,0,0,7.16,14.32ZM79.77,156.72a23.73,23.73,0,0,0-9.6-15.95,24.86,24.86,0,0,0-34.11,4.7,23.63,23.63,0,0,0-3.57,6.46,8,8,0,1,0,15,5.47,7.84,7.84,0,0,1,1.18-2.13,8.76,8.76,0,0,1,12-1.59A7.91,7.91,0,0,1,63.93,159a7.64,7.64,0,0,1-1.57,5.78,1,1,0,0,0-.08.11L33.59,203.21A8,8,0,0,0,40,216H72a8,8,0,0,0,0-16H56l19.08-25.53A23.47,23.47,0,0,0,79.77,156.72Z"
    // check-square (regular)
    const val TASK = "M173.66,98.34a8,8,0,0,1,0,11.32l-56,56a8,8,0,0,1-11.32,0l-24-24a8,8,0,0,1,11.32-11.32L112,148.69l50.34-50.35A8,8,0,0,1,173.66,98.34ZM224,48V208a16,16,0,0,1-16,16H48a16,16,0,0,1-16-16V48A16,16,0,0,1,48,32H208A16,16,0,0,1,224,48ZM208,208V48H48V208H208Z"
    // quotes (regular)
    const val QUOTE = "M100,56H40A16,16,0,0,0,24,72v64a16,16,0,0,0,16,16h60v8a32,32,0,0,1-32,32,8,8,0,0,0,0,16,48.05,48.05,0,0,0,48-48V72A16,16,0,0,0,100,56Zm0,80H40V72h60ZM216,56H156a16,16,0,0,0-16,16v64a16,16,0,0,0,16,16h60v8a32,32,0,0,1-32,32,8,8,0,0,0,0,16,48.05,48.05,0,0,0,48-48V72A16,16,0,0,0,216,56Zm0,80H156V72h60Z"
    // lock-simple (regular)
    const val LOCK = "M208,80H176V56a48,48,0,0,0-96,0V80H48A16,16,0,0,0,32,96V208a16,16,0,0,0,16,16H208a16,16,0,0,0,16-16V96A16,16,0,0,0,208,80ZM96,56a32,32,0,0,1,64,0V80H96ZM208,208H48V96H208V208Z"
    // image (regular)
    const val IMAGE = "M216,40H40A16,16,0,0,0,24,56V200a16,16,0,0,0,16,16H216a16,16,0,0,0,16-16V56A16,16,0,0,0,216,40Zm0,16V158.75l-26.07-26.06a16,16,0,0,0-22.63,0l-20,20-44-44a16,16,0,0,0-22.62,0L40,149.37V56ZM40,172l52-52,80,80H40Zm176,28H194.63l-36-36,20-20L216,181.38V200ZM144,100a12,12,0,1,1,12,12A12,12,0,0,1,144,100Z"
}

@Composable
fun QIcon(d: String, size: Dp, color: Color, modifier: Modifier = Modifier) {
    val path = remember(d) { PathParser().parsePathString(d).toPath() }
    Canvas(modifier.size(size)) {
        val s = this.size.width / 256f
        scale(s, s, pivot = Offset.Zero) {
            drawPath(path, color)
        }
    }
}

/** A click target that shows the design's hover color while pressed, with no ripple. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun Modifier.tap(
    shape: Shape = RectangleShape,
    pressed: Color = LocalQ.current.hover,
    enabled: Boolean = true,
    onLongClick: (() -> Unit)? = null,
    onClick: () -> Unit,
): Modifier {
    val src = remember { MutableInteractionSource() }
    val isPressed by src.collectIsPressedAsState()
    return this
        .clip(shape)
        .background(if (isPressed) pressed else Color.Transparent)
        .combinedClickable(interactionSource = src, indication = null, enabled = enabled, onLongClick = onLongClick, onClick = onClick)
}

@Composable
fun Txt(
    text: String,
    size: TextUnit,
    color: Color,
    modifier: Modifier = Modifier,
    weight: FontWeight = FontWeight.W400,
    family: FontFamily = QFonts.sans,
    lineHeight: TextUnit = TextUnit.Unspecified,
    italic: Boolean = false,
    maxLines: Int = Int.MAX_VALUE,
    align: TextAlign? = null,
    letterSpacing: TextUnit = TextUnit.Unspecified,
) {
    BasicText(
        text = text,
        modifier = modifier,
        style = TextStyle(
            color = color, fontSize = size, fontWeight = weight, fontFamily = family, lineHeight = lineHeight,
            fontStyle = if (italic) FontStyle.Italic else FontStyle.Normal, textAlign = align ?: TextAlign.Unspecified,
            letterSpacing = letterSpacing,
        ),
        maxLines = maxLines,
        overflow = if (maxLines == Int.MAX_VALUE) TextOverflow.Clip else TextOverflow.Ellipsis,
    )
}

@Composable
fun Mono(text: String, size: TextUnit, color: Color, modifier: Modifier = Modifier, maxLines: Int = Int.MAX_VALUE, lineHeight: TextUnit = TextUnit.Unspecified) =
    Txt(text, size, color, modifier, family = QFonts.plex, maxLines = maxLines, lineHeight = lineHeight)

/** A 32 dp icon button from the title bar. */
@Composable
fun IconBtn(d: String, iconSize: Dp = 16.dp, size: Dp = 32.dp, color: Color = LocalQ.current.mute, onClick: () -> Unit) {
    Box(Modifier.size(size).tap(RoundedCornerShape(6.dp), onClick = onClick), contentAlignment = Alignment.Center) {
        QIcon(d, iconSize, color)
    }
}

class SegOpt(val label: String, val on: Boolean, val onClick: () -> Unit)

/** The segmented control. [small] is the source and preview switch in the title bar. */
@Composable
fun Segmented(opts: List<SegOpt>, small: Boolean = false, modifier: Modifier = Modifier) {
    val c = LocalQ.current
    Row(
        modifier
            .clip(RoundedCornerShape(if (small) 7.dp else 8.dp))
            .background(if (small) c.hover else c.side)
            .padding(2.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        opts.forEach { o ->
            val shape = RoundedCornerShape(if (small) 5.dp else 6.dp)
            Box(
                Modifier
                    .height(if (small) 24.dp else 28.dp)
                    .then(if (o.on) Modifier.shadow(1.dp, shape, ambientColor = Shadow, spotColor = Shadow).background(c.bg, shape) else Modifier)
                    .tap(shape, pressed = Color.Transparent, onClick = o.onClick)
                    .padding(horizontal = if (small) 9.dp else 12.dp),
                contentAlignment = Alignment.Center,
            ) {
                if (small) {
                    Mono(o.label, 11.sp, if (o.on) c.ink else c.mute)
                } else {
                    Txt(o.label, 12.5.sp, if (o.on) c.ink else c.mute, maxLines = 1)
                }
            }
        }
    }
}

/** The on and off switch from the settings panel. */
@Composable
fun Toggle(on: Boolean, onClick: () -> Unit) {
    val c = LocalQ.current
    Box(
        Modifier
            .size(36.dp, 20.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(if (on) c.button else c.mark)
            .tap(RoundedCornerShape(10.dp), pressed = Color.Transparent, onClick = onClick)
            .padding(2.dp),
        contentAlignment = if (on) Alignment.CenterEnd else Alignment.CenterStart,
    ) {
        Box(Modifier.size(16.dp).shadow(1.dp, CircleShape).background(Color.White, CircleShape))
    }
}

@Composable
fun PrimaryButton(label: String, modifier: Modifier = Modifier, height: Dp = 32.dp, hPad: Dp = 14.dp, size: TextUnit = 13.sp, trailing: (@Composable RowScope.() -> Unit)? = null, onClick: () -> Unit) {
    val src = remember { MutableInteractionSource() }
    val pressed by src.collectIsPressedAsState()
    Row(
        modifier
            .height(height)
            .clip(RoundedCornerShape(8.dp))
            .background(if (pressed) LocalQ.current.buttonPressed else LocalQ.current.button)
            .combinedClickable(interactionSource = src, indication = null, onClick = onClick)
            .padding(horizontal = hPad),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Txt(label, size, LocalQ.current.onAcc, weight = FontWeight.W500, maxLines = 1)
        trailing?.invoke(this)
    }
}

@Composable
fun OutlineButton(label: String, modifier: Modifier = Modifier, height: Dp = 30.dp, hPad: Dp = 12.dp, size: TextUnit = 12.5.sp, color: Color = LocalQ.current.ink, onClick: () -> Unit) {
    val c = LocalQ.current
    Box(
        modifier
            .height(height)
            .border(1.dp, c.line, RoundedCornerShape(7.dp))
            .tap(RoundedCornerShape(7.dp), onClick = onClick)
            .padding(horizontal = hPad),
        contentAlignment = Alignment.Center,
    ) {
        Txt(label, size, color, maxLines = 1)
    }
}

/** A small mono label such as "outline" or "recent". */
@Composable
fun Label(text: String, modifier: Modifier = Modifier) = Mono(text, 11.sp, LocalQ.current.mute, modifier)

@Composable
fun Hairline(modifier: Modifier = Modifier) {
    Box(modifier.height(1.dp).background(LocalQ.current.line))
}

@Composable
fun VLine(modifier: Modifier = Modifier) {
    Box(modifier.width(1.dp).background(LocalQ.current.line))
}

// ---- Text decorations ------------------------------------------------------------------------

enum class DecoKind { Underline, Box, LineBg }

/**
 * A decoration that a span style can't draw: a link underline with an offset and its own color,
 * a rounded background behind a tag or inline code, or a full-width background behind a code line.
 * [em] is the font size of the decorated text, relative to the base text style.
 */
class Deco(
    val start: Int,
    val end: Int,
    val color: Color,
    val kind: DecoKind,
    val dashed: Boolean = false,
    val radius: Dp = 0.dp,
    val em: Float = 1f,
    val padV: Float = 0f,
)

class Rich(val text: androidx.compose.ui.text.AnnotatedString, val decos: List<Deco>)

fun DrawScope.drawDecos(l: TextLayoutResult, decos: List<Deco>, basePx: Float) {
    val len = l.layoutInput.text.length
    val one = 1.dp.toPx()
    for (d in decos) {
        if (d.start > len || d.end > len || d.end < d.start) continue
        when (d.kind) {
            DecoKind.LineBg -> {
                val l0 = l.getLineForOffset(d.start)
                val l1 = l.getLineForOffset(max(d.start, d.end - 1))
                val top = l.getLineTop(l0)
                val bottom = l.getLineBottom(l1)
                drawRect(d.color, Offset(0f, top), Size(size.width, bottom - top))
            }
            else -> {
                if (d.end <= d.start) continue
                val l0 = l.getLineForOffset(d.start)
                val l1 = l.getLineForOffset(d.end - 1)
                val fs = basePx * d.em
                for (li in l0..l1) {
                    val s = max(d.start, l.getLineStart(li))
                    val e = min(d.end, l.getLineEnd(li, visibleEnd = true))
                    if (e <= s) continue
                    val x0 = l.getBoundingBox(s).left
                    val x1 = l.getBoundingBox(e - 1).right
                    val base = l.getLineBaseline(li)
                    if (d.kind == DecoKind.Underline) {
                        val y = base + 3 * one
                        drawLine(
                            d.color, Offset(x0, y), Offset(x1, y), strokeWidth = one,
                            pathEffect = if (d.dashed) PathEffect.dashPathEffect(floatArrayOf(3 * one, 2 * one)) else null,
                        )
                    } else {
                        val pad = fs * d.padV
                        val top = base - fs * 0.98f - pad
                        val bottom = base + fs * 0.3f + pad
                        val r = d.radius.toPx()
                        drawRoundRect(d.color, Offset(x0, top), Size(x1 - x0, bottom - top), CornerRadius(r, r))
                    }
                }
            }
        }
    }
}

