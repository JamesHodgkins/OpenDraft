using System;

namespace OpenDraft.ODCore.ODData
{
    public class ODColour
    {
        // Basic colors
        public static ODColour Red => new(255, 0, 0);
        public static ODColour Green => new(0, 255, 0);
        public static ODColour Blue => new(0, 0, 255);
        public static ODColour White => new(255, 255, 255);
        public static ODColour Black => new(0, 0, 0);
        public static ODColour Transparent => new(0, 0, 0, 0);

        // Secondary colors
        public static ODColour Yellow => new(255, 255, 0);
        public static ODColour Cyan => new(0, 255, 255);
        public static ODColour Magenta => new(255, 0, 255);

        // Grayscale variants
        public static ODColour Gray => new(128, 128, 128);
        public static ODColour LightGray => new(211, 211, 211);
        public static ODColour DarkGray => new(169, 169, 169);
        public static ODColour Silver => new(192, 192, 192);
        public static ODColour DimGray => new(105, 105, 105);
        public static ODColour SlateGray => new(112, 128, 144);

        // Red family
        public static ODColour DarkRed => new(139, 0, 0);
        public static ODColour FireBrick => new(178, 34, 34);
        public static ODColour Crimson => new(220, 20, 60);
        public static ODColour IndianRed => new(205, 92, 92);
        public static ODColour LightCoral => new(240, 128, 128);
        public static ODColour Salmon => new(250, 128, 114);
        public static ODColour DarkSalmon => new(233, 150, 122);
        public static ODColour LightSalmon => new(255, 160, 122);

        // Green family
        public static ODColour DarkGreen => new(0, 100, 0);
        public static ODColour ForestGreen => new(34, 139, 34);
        public static ODColour SeaGreen => new(46, 139, 87);
        public static ODColour MediumSeaGreen => new(60, 179, 113);
        public static ODColour LimeGreen => new(50, 205, 50);
        public static ODColour SpringGreen => new(0, 255, 127);
        public static ODColour MediumSpringGreen => new(0, 250, 154);
        public static ODColour PaleGreen => new(152, 251, 152);
        public static ODColour LightGreen => new(144, 238, 144);
        public static ODColour LawnGreen => new(124, 252, 0);
        public static ODColour Chartreuse => new(127, 255, 0);
        public static ODColour Olive => new(128, 128, 0);
        public static ODColour OliveDrab => new(107, 142, 35);
        public static ODColour DarkOliveGreen => new(85, 107, 47);

        // Blue family
        public static ODColour DarkBlue => new(0, 0, 139);
        public static ODColour MediumBlue => new(0, 0, 205);
        public static ODColour RoyalBlue => new(65, 105, 225);
        public static ODColour SteelBlue => new(70, 130, 180);
        public static ODColour DodgerBlue => new(30, 144, 255);
        public static ODColour DeepSkyBlue => new(0, 191, 255);
        public static ODColour CornflowerBlue => new(100, 149, 237);
        public static ODColour SkyBlue => new(135, 206, 235);
        public static ODColour LightSkyBlue => new(135, 206, 250);
        public static ODColour LightSteelBlue => new(176, 196, 222);
        public static ODColour LightBlue => new(173, 216, 230);
        public static ODColour PowderBlue => new(176, 224, 230);
        public static ODColour Navy => new(0, 0, 128);
        public static ODColour MidnightBlue => new(25, 25, 112);
        public static ODColour DarkSlateBlue => new(72, 61, 139);

        // Purple/Magenta family
        public static ODColour Purple => new(128, 0, 128);
        public static ODColour DarkMagenta => new(139, 0, 139);
        public static ODColour DarkViolet => new(148, 0, 211);
        public static ODColour BlueViolet => new(138, 43, 226);
        public static ODColour MediumPurple => new(147, 112, 219);
        public static ODColour MediumOrchid => new(186, 85, 211);
        public static ODColour Orchid => new(218, 112, 214);
        public static ODColour Violet => new(238, 130, 238);
        public static ODColour Plum => new(221, 160, 221);
        public static ODColour Thistle => new(216, 191, 216);
        public static ODColour Lavender => new(230, 230, 250);

        // Orange/Brown family
        public static ODColour Orange => new(255, 165, 0);
        public static ODColour DarkOrange => new(255, 140, 0);
        public static ODColour Coral => new(255, 127, 80);
        public static ODColour Tomato => new(255, 99, 71);
        public static ODColour OrangeRed => new(255, 69, 0);
        public static ODColour Gold => new(255, 215, 0);
        public static ODColour YellowGold => new(255, 223, 0);
        public static ODColour Goldenrod => new(218, 165, 32);
        public static ODColour DarkGoldenrod => new(184, 134, 11);
        public static ODColour Chocolate => new(210, 105, 30);
        public static ODColour SaddleBrown => new(139, 69, 19);
        public static ODColour Sienna => new(160, 82, 45);
        public static ODColour Brown => new(165, 42, 42);
        public static ODColour Maroon => new(128, 0, 0);
        public static ODColour Tan => new(210, 180, 140);
        public static ODColour RosyBrown => new(188, 143, 143);
        public static ODColour Peru => new(205, 133, 63);
        public static ODColour BurlyWood => new(222, 184, 135);
        public static ODColour Wheat => new(245, 222, 179);
        public static ODColour SandyBrown => new(244, 164, 96);

        // Pink family
        public static ODColour Pink => new(255, 192, 203);
        public static ODColour LightPink => new(255, 182, 193);
        public static ODColour HotPink => new(255, 105, 180);
        public static ODColour DeepPink => new(255, 20, 147);
        public static ODColour PaleVioletRed => new(219, 112, 147);
        public static ODColour MediumVioletRed => new(199, 21, 133);

        // Yellow family
        public static ODColour LightYellow => new(255, 255, 224);
        public static ODColour LemonChiffon => new(255, 250, 205);
        public static ODColour LightGoldenrodYellow => new(250, 250, 210);
        public static ODColour PapayaWhip => new(255, 239, 213);
        public static ODColour Moccasin => new(255, 228, 181);
        public static ODColour PeachPuff => new(255, 218, 185);
        public static ODColour PaleGoldenrod => new(238, 232, 170);
        public static ODColour Khaki => new(240, 230, 140);
        public static ODColour DarkKhaki => new(189, 183, 107);

        // Special/System colors
        public static ODColour AliceBlue => new(240, 248, 255);
        public static ODColour GhostWhite => new(248, 248, 255);
        public static ODColour Azure => new(240, 255, 255);
        public static ODColour MintCream => new(245, 255, 250);
        public static ODColour Honeydew => new(240, 255, 240);
        public static ODColour Ivory => new(255, 255, 240);
        public static ODColour Beige => new(245, 245, 220);
        public static ODColour OldLace => new(253, 245, 230);
        public static ODColour FloralWhite => new(255, 250, 240);
        public static ODColour Seashell => new(255, 245, 238);
        public static ODColour Snow => new(255, 250, 250);
        public static ODColour Linen => new(250, 240, 230);
        public static ODColour AntiqueWhite => new(250, 235, 215);
        public static ODColour NavajoWhite => new(255, 222, 173);
        public static ODColour Cornsilk => new(255, 248, 220);
        public static ODColour BlanchedAlmond => new(255, 235, 205);
        public static ODColour Bisque => new(255, 228, 196);



        public byte R { get; set; }
        public byte G { get; set; }
        public byte B { get; set; }
        public byte A { get; set; }

        public ODColour()
        {
            R = 0;
            G = 0;
            B = 0;
            A = 255;
        }

        public ODColour(byte r, byte g, byte b)
        {
            R = r;
            G = g;
            B = b;
            A = 255;
        }

        public ODColour(byte r, byte g, byte b, byte a)
        {
            R = r;
            G = g;
            B = b;
            A = a;
        }

        public ODColour(string hex)
        {
            SetFromHex(hex);
        }

        public ODColour(ODColour color)
        {
            R = color.R;
            G = color.G;
            B = color.B;
            A = color.A;
        }

        public ODColour(Avalonia.Media.Color color)
        {
            R = color.R;
            G = color.G;
            B = color.B;
            A = color.A;
        }

        public override string ToString()
        {
            return $"ODColour(R: {R}, G: {G}, B: {B}, A: {A})";
        }

        public void SetRGB(byte r, byte g, byte b)
        {
            R = r;
            G = g;
            B = b;
        }

        public void SetAlpha(byte a)
        {
            A = a;
        }

        public void SetRGBA(byte r, byte g, byte b, byte a)
        {
            R = r;
            G = g;
            B = b;
            A = a;
        }

        public void SetFromHex(string hex)
        {
            if (hex.StartsWith("#"))
            {
                hex = hex.Substring(1);
            }
            if (hex.Length == 6)
            {
                R = Convert.ToByte(hex.Substring(0, 2), 16);
                G = Convert.ToByte(hex.Substring(2, 2), 16);
                B = Convert.ToByte(hex.Substring(4, 2), 16);
                A = 255; // Default alpha
            }
            else if (hex.Length == 8)
            {
                R = Convert.ToByte(hex.Substring(0, 2), 16);
                G = Convert.ToByte(hex.Substring(2, 2), 16);
                B = Convert.ToByte(hex.Substring(4, 2), 16);
                A = Convert.ToByte(hex.Substring(6, 2), 16);
            }
            else
            {
                throw new ArgumentException("Hex string must be in the format RRGGBB or RRGGBBAA");
            }
        }

        public void SetFromHSL(float h, float s, float l, byte a = 255)
        {
            // Normalize inputs
            h = (h % 360 + 360) % 360; // Wrap hue to 0-360
            s = Math.Clamp(s, 0, 100);
            l = Math.Clamp(l, 0, 100);

            s /= 100f;
            l /= 100f;

            float c = (1 - Math.Abs(2 * l - 1)) * s;
            float x = c * (1 - Math.Abs((h / 60) % 2 - 1));
            float m = l - c / 2;

            float r, g, b;

            if (h >= 0 && h < 60) { r = c; g = x; b = 0; }
            else if (h >= 60 && h < 120) { r = x; g = c; b = 0; }
            else if (h >= 120 && h < 180) { r = 0; g = c; b = x; }
            else if (h >= 180 && h < 240) { r = 0; g = x; b = c; }
            else if (h >= 240 && h < 300) { r = x; g = 0; b = c; }
            else { r = c; g = 0; b = x; }

            R = (byte)((r + m) * 255);
            G = (byte)((g + m) * 255);
            B = (byte)((b + m) * 255);
            A = a;
        }

        public string ToHex()
        {
            return A == 255
                ? $"#{R:X2}{G:X2}{B:X2}"
                : $"#{R:X2}{G:X2}{B:X2}{A:X2}";
        }

        public Avalonia.Media.Color ToAvaloniaColor()
        {
            return Avalonia.Media.Color.FromArgb(A, R, G, B);
        }

        public ODColour Lighten(float factor)
        {
            factor = Math.Clamp(factor, 0, 1);
            return new ODColour(
                (byte)Math.Min(255, R + (255 - R) * factor),
                (byte)Math.Min(255, G + (255 - G) * factor),
                (byte)Math.Min(255, B + (255 - B) * factor),
                A
            );
        }

        public ODColour Darken(float factor)
        {
            factor = Math.Clamp(factor, 0, 1);
            return new ODColour(
                (byte)Math.Max(0, R * (1 - factor)),
                (byte)Math.Max(0, G * (1 - factor)),
                (byte)Math.Max(0, B * (1 - factor)),
                A
            );
        }

        public ODColour Invert()
        {
            return new ODColour(
                (byte)(255 - R),
                (byte)(255 - G),
                (byte)(255 - B),
                A
            );
        }

        public (float Hue, float Saturation, float Lightness) ToHSL()
        {
            float r = R / 255f;
            float g = G / 255f;
            float b = B / 255f;

            float max = Math.Max(r, Math.Max(g, b));
            float min = Math.Min(r, Math.Min(g, b));
            float delta = max - min;

            float hue = 0;
            float saturation = 0;
            float lightness = (max + min) / 2;

            if (delta != 0)
            {
                saturation = lightness < 0.5 ? delta / (max + min) : delta / (2 - max - min);

                if (max == r)
                    hue = (g - b) / delta + (g < b ? 6 : 0);
                else if (max == g)
                    hue = (b - r) / delta + 2;
                else
                    hue = (r - g) / delta + 4;

                hue *= 60;
            }

            return (hue, saturation * 100, lightness * 100);
        }

    }
}
