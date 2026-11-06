// NovaTheme.swift
// The look of NovaKey's windows (Settings > Appearance): the design tokens
// every SwiftUI view reads from the environment, and the small primitives
// that differ in shape rather than colour between themes.
//
// Two themes, mirroring the Windows app's picker:
// - Modern: the dark, card-based look (the default).
// - Platinum: the gray, bevelled look of classic Mac OS -- etched group
//   boxes, square check boxes, pinstriped header, push buttons with a
//   highlight and shadow edge. The system window frame is kept; only the
//   content is themed, so the traffic lights stay where macOS puts them.

import SwiftUI

struct NovaTheme {
    let kind: SettingsTheme

    // Surfaces
    let windowBG: Color
    let cardBG: Color
    let cardStroke: Color
    let fieldBG: Color
    let chipBG: Color
    let pillOffBG: Color
    let divider: Color
    let hoverBG: Color
    let popoverBG: Color

    // Text
    let sectionLabel: Color
    let textPrimary: Color
    let textSecondary: Color
    let danger: Color

    // Accent: the lit language button, the chosen segment, a checked box.
    let accent: Color
    let accentGradient: LinearGradient
    let accentGlow: Color

    // Platinum bevels (unused by Modern).
    let bevelLight: Color
    let bevelShadow: Color
    let bevelDark: Color

    /// The AppKit appearance the hosting window should take, so native
    /// controls (text fields, menus) match the content.
    let appearance: NSAppearance.Name

    var isRetro: Bool { kind == .platinum }
    var cardRadius: CGFloat { isRetro ? 0 : 14 }
    var controlRadius: CGFloat { isRetro ? 4 : 7 }

    /// The window background as an NSColor, for `NSWindow.backgroundColor`.
    var windowColor: NSColor { NSColor(windowBG) }

    /// The theme the user chose in Settings.
    static func current() -> NovaTheme {
        switch AppSettings.shared.settingsTheme {
        case .modern: return .modern
        case .platinum: return .platinum
        }
    }

    static let modern: NovaTheme = {
        let gradient = LinearGradient(
            colors: [
                Color(red: 0.90, green: 0.15, blue: 0.20),
                Color(red: 0.98, green: 0.43, blue: 0.16),
                Color(red: 1.0, green: 0.78, blue: 0.28),
            ],
            startPoint: .leading, endPoint: .trailing
        )
        return NovaTheme(
            kind: .modern,
            windowBG: Color(red: 0.11, green: 0.11, blue: 0.12),
            cardBG: Color(red: 0.16, green: 0.16, blue: 0.18),
            cardStroke: Color.white.opacity(0.06),
            fieldBG: Color.white.opacity(0.05),
            chipBG: Color.white.opacity(0.09),
            pillOffBG: Color.white.opacity(0.08),
            divider: Color.white.opacity(0.06),
            hoverBG: Color.white.opacity(0.06),
            popoverBG: Color(red: 0.13, green: 0.12, blue: 0.14),
            sectionLabel: Color(red: 1.0, green: 0.72, blue: 0.27),
            textPrimary: .white,
            textSecondary: Color.white.opacity(0.55),
            danger: Color(red: 1.0, green: 0.42, blue: 0.42),
            accent: Color(red: 0.98, green: 0.43, blue: 0.16),
            accentGradient: gradient,
            accentGlow: Color.red.opacity(0.45),
            bevelLight: .white,
            bevelShadow: Color.white.opacity(0.2),
            bevelDark: .black,
            appearance: .darkAqua
        )
    }()

    /// Mac OS 8/9 Platinum: the same values the Windows app uses.
    static let platinum: NovaTheme = {
        let accent = Color(red: 102 / 255, green: 102 / 255, blue: 204 / 255)
        return NovaTheme(
            kind: .platinum,
            windowBG: Color(white: 221 / 255),
            cardBG: Color(white: 221 / 255),
            cardStroke: Color(white: 136 / 255),
            fieldBG: .white,
            chipBG: .white,
            pillOffBG: Color(white: 221 / 255),
            divider: Color(white: 136 / 255),
            hoverBG: accent,
            popoverBG: Color(white: 221 / 255),
            sectionLabel: .black,
            textPrimary: .black,
            textSecondary: Color(white: 85 / 255),
            danger: Color(red: 204 / 255, green: 0, blue: 0),
            accent: accent,
            accentGradient: LinearGradient(colors: [accent, accent], startPoint: .leading, endPoint: .trailing),
            accentGlow: .clear,
            bevelLight: .white,
            bevelShadow: Color(white: 136 / 255),
            bevelDark: Color(white: 51 / 255),
            appearance: .aqua
        )
    }()
}

// MARK: - Environment

private struct NovaThemeKey: EnvironmentKey {
    static let defaultValue: NovaTheme = .modern
}

extension EnvironmentValues {
    var novaTheme: NovaTheme {
        get { self[NovaThemeKey.self] }
        set { self[NovaThemeKey.self] = newValue }
    }
}

/// Wraps a window's root view: supplies the current theme to the
/// environment and follows changes made in Settings while the window is
/// open.
struct ThemedRoot<Content: View>: View {
    @State private var theme: NovaTheme = NovaTheme.current()
    @ViewBuilder let content: () -> Content

    var body: some View {
        content()
            .environment(\.novaTheme, theme)
            .onReceive(NotificationCenter.default.publisher(for: .novaKeySettingsChanged)) { _ in
                if theme.kind != AppSettings.shared.settingsTheme { theme = NovaTheme.current() }
            }
    }
}

// MARK: - Primitives

/// A one-pixel highlight along the top and left edges and a shadow along
/// the bottom and right (or the reverse when sunken). Overlay it on a face.
struct Bevel: View {
    let light: Color
    let shadow: Color
    var raised: Bool = true
    var inset: CGFloat = 1.5

    var body: some View {
        GeometryReader { g in
            let w = g.size.width, h = g.size.height, i = inset
            Path { p in
                p.move(to: CGPoint(x: i, y: h - i))
                p.addLine(to: CGPoint(x: i, y: i))
                p.addLine(to: CGPoint(x: w - i, y: i))
            }
            .stroke(raised ? light : shadow, lineWidth: 1)
            Path { p in
                p.move(to: CGPoint(x: w - i, y: i))
                p.addLine(to: CGPoint(x: w - i, y: h - i))
                p.addLine(to: CGPoint(x: i, y: h - i))
            }
            .stroke(raised ? shadow : light, lineWidth: 1)
        }
        .allowsHitTesting(false)
    }
}

/// The Platinum title-bar pinstripes: a ridge (highlight over shadow) on
/// every third line.
struct PlatinumStripes: View {
    let light: Color
    let shadow: Color

    var body: some View {
        Canvas { ctx, size in
            var y: CGFloat = 4
            while y + 2 <= size.height - 4 {
                ctx.fill(Path(CGRect(x: 0, y: y, width: size.width, height: 1)), with: .color(light))
                ctx.fill(Path(CGRect(x: 0, y: y + 1, width: size.width, height: 1)), with: .color(shadow))
                y += 3
            }
        }
        .allowsHitTesting(false)
    }
}

/// A window header: the plain title in Modern, the title on a patch cut out
/// of pinstripes in Platinum.
struct NovaHeader: View {
    @Environment(\.novaTheme) private var theme
    let title: String

    var body: some View {
        if theme.isRetro {
            ZStack {
                PlatinumStripes(light: theme.bevelLight, shadow: theme.bevelShadow)
                    .frame(height: 20)
                    .padding(.horizontal, 6)
                Text(title)
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(theme.textPrimary)
                    .padding(.horizontal, 8)
                    .background(theme.windowBG)
            }
            .frame(maxWidth: .infinity)
        } else {
            Text(title)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(theme.textPrimary)
                .frame(maxWidth: .infinity)
                .padding(.top, 6)
        }
    }
}

/// Push button: a soft rounded chip in Modern, a bevelled button that
/// darkens while held in Platinum. `selected` keeps it in the pressed look
/// (the chosen segment of a NovaSegmented).
struct NovaButtonStyle: ButtonStyle {
    @Environment(\.novaTheme) private var theme
    @Environment(\.isEnabled) private var isEnabled
    var selected: Bool = false

    func makeBody(configuration: Configuration) -> some View {
        let down = configuration.isPressed || selected
        Group {
            if theme.isRetro {
                configuration.label
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(down ? Color.white : theme.textPrimary)
                    .padding(.horizontal, 12)
                    .frame(height: 22)
                    .background(
                        RoundedRectangle(cornerRadius: theme.controlRadius)
                            .fill(down ? theme.bevelDark : theme.cardBG)
                    )
                    .overlay(
                        Bevel(light: down ? theme.bevelShadow : theme.bevelLight,
                              shadow: down ? theme.bevelDark : theme.bevelShadow)
                            .clipShape(RoundedRectangle(cornerRadius: theme.controlRadius))
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: theme.controlRadius)
                            .stroke(Color.black, lineWidth: 1)
                    )
            } else {
                configuration.label
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(theme.textPrimary)
                    .padding(.horizontal, 12)
                    .frame(height: 24)
                    .background(
                        RoundedRectangle(cornerRadius: theme.controlRadius, style: .continuous)
                            .fill(selected ? AnyShapeStyle(theme.accentGradient)
                                  : AnyShapeStyle(Color.white.opacity(configuration.isPressed ? 0.16 : 0.09)))
                    )
            }
        }
        .opacity(isEnabled ? 1 : 0.4)
        .contentShape(Rectangle())
    }
}

/// A row of mutually exclusive choices: a pill group in Modern, a row of
/// push buttons with the chosen one held down in Platinum.
struct NovaSegmented<Value: Hashable>: View {
    @Environment(\.novaTheme) private var theme
    let options: [(label: String, value: Value)]
    @Binding var selection: Value

    var body: some View {
        HStack(spacing: theme.isRetro ? 6 : 2) {
            ForEach(Array(options.enumerated()), id: \.offset) { _, option in
                Button(option.label) { selection = option.value }
                    .buttonStyle(NovaButtonStyle(selected: selection == option.value))
            }
        }
        .padding(theme.isRetro ? 0 : 3)
        .background(
            RoundedRectangle(cornerRadius: theme.controlRadius + 3, style: .continuous)
                .fill(theme.isRetro ? Color.clear : theme.pillOffBG)
        )
    }
}

/// On/off control: a gradient switch in Modern, a square check box with a
/// black tick in Platinum.
struct NovaToggle: View {
    @Environment(\.novaTheme) private var theme
    @Binding var isOn: Bool

    var body: some View {
        Group {
            if theme.isRetro {
                ZStack {
                    Rectangle()
                        .fill(theme.fieldBG)
                        .frame(width: 14, height: 14)
                        .overlay(Rectangle().stroke(Color.black, lineWidth: 1))
                        .overlay(Bevel(light: theme.bevelLight, shadow: theme.bevelShadow, raised: false))
                    if isOn {
                        Path { p in
                            p.move(to: CGPoint(x: 3, y: 7))
                            p.addLine(to: CGPoint(x: 6, y: 10.5))
                            p.addLine(to: CGPoint(x: 11.5, y: 3.5))
                        }
                        .stroke(Color.black, style: StrokeStyle(lineWidth: 2, lineCap: .round, lineJoin: .round))
                        .frame(width: 14, height: 14)
                    }
                }
                .frame(width: 44, height: 24, alignment: .trailing)
                .padding(.trailing, 4)
            } else {
                ZStack(alignment: isOn ? .trailing : .leading) {
                    Capsule()
                        .fill(isOn ? AnyShapeStyle(theme.accentGradient) : AnyShapeStyle(Color.white.opacity(0.12)))
                        .frame(width: 44, height: 24)
                    Circle()
                        .fill(Color.white)
                        .frame(width: 18, height: 18)
                        .padding(3)
                        .shadow(color: .black.opacity(0.25), radius: 1.5, y: 1)
                }
            }
        }
        .contentShape(Rectangle())
        .onTapGesture {
            withAnimation(theme.isRetro ? nil : .spring(response: 0.22, dampingFraction: 0.8)) {
                isOn.toggle()
            }
        }
    }
}

/// Section container: a rounded card in Modern, an etched group box with
/// the label set into its top edge in Platinum.
struct NovaCard<Content: View>: View {
    @Environment(\.novaTheme) private var theme
    let title: String
    @ViewBuilder let content: () -> Content

    var body: some View {
        if theme.isRetro {
            ZStack(alignment: .topLeading) {
                VStack(alignment: .leading, spacing: 12) {
                    content()
                }
                .padding(16)
                .padding(.top, 6)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(
                    ZStack {
                        Rectangle().stroke(theme.bevelLight, lineWidth: 1).offset(x: 1, y: 1)
                        Rectangle().stroke(theme.cardStroke, lineWidth: 1)
                    }
                )
                .padding(.top, 8)
                Text(title)
                    .font(.system(size: 11, weight: .bold))
                    .foregroundStyle(theme.sectionLabel)
                    .padding(.horizontal, 4)
                    .background(theme.windowBG)
                    .padding(.leading, 10)
                    .padding(.top, 1)
            }
        } else {
            VStack(alignment: .leading, spacing: 12) {
                Text(title)
                    .font(.system(size: 11, weight: .semibold))
                    .tracking(1.2)
                    .foregroundStyle(theme.sectionLabel)
                content()
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: theme.cardRadius, style: .continuous)
                    .fill(theme.cardBG)
                    .overlay(
                        RoundedRectangle(cornerRadius: theme.cardRadius, style: .continuous)
                            .stroke(theme.cardStroke, lineWidth: 1)
                    )
            )
        }
    }
}

/// A text field in the theme's style (rounded in Modern, square in Platinum).
struct NovaTextField: View {
    @Environment(\.novaTheme) private var theme
    let placeholder: String
    @Binding var text: String

    var body: some View {
        if theme.isRetro {
            TextField(placeholder, text: $text).textFieldStyle(.squareBorder)
        } else {
            TextField(placeholder, text: $text).textFieldStyle(.roundedBorder)
        }
    }
}
