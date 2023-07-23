#ifndef OD_SY_COLOUR_CPP
#define OD_SY_COLOUR_CPP

/**************************************************************************************
* OpenDraft:    System Colour Class                                                   *
*-------------------------------------------------------------------------------------*
* Filename:     OdColour.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 21, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A colour class for the OpenDraft framework.                                       *
***************************************************************************************/



#include "System/Utils/OdColour.h"

namespace OD
{
	namespace System
	{
		// Get colour by index
		const OdColour OdColour::colourByIndex(int index)
		{
			OdColour result = OdColour();

			switch (index)
			{
				case 0:  result.setRGB(0, 0, 0); break;
				case 1:  result.setRGB(255, 0, 0); break;
				case 2:  result.setRGB(255, 255, 0); break;
				case 3:  result.setRGB(0, 255, 0); break;
				case 4:  result.setRGB(0, 255, 255); break;
				case 5:  result.setRGB(0, 0, 255); break;
				case 6:  result.setRGB(255, 0, 255); break;
				case 7:  result.setRGB(255, 255, 255); break;
				case 8:  result.setRGB(65, 65, 65); break;
				case 9:  result.setRGB(128, 128, 128); break;
				case 10: result.setRGB(255, 0, 0); break;
				case 11: result.setRGB(255, 170, 170); break;
				case 12: result.setRGB(189, 0, 0); break;
				case 13: result.setRGB(189, 126, 126); break;
				case 14: result.setRGB(129, 0, 0); break;
				case 15: result.setRGB(129, 86, 86); break;
				case 16: result.setRGB(104, 0, 0); break;
				case 17: result.setRGB(104, 69, 69); break;
				case 18: result.setRGB(79, 0, 0); break;
				case 19: result.setRGB(79, 53, 53); break;
				case 20: result.setRGB(255, 63, 0); break;
				case 21: result.setRGB(255, 191, 170); break;
				case 22: result.setRGB(189, 46, 0); break;
				case 23: result.setRGB(189, 141, 126); break;
				case 24: result.setRGB(129, 31, 0); break;
				case 25: result.setRGB(129, 96, 86); break;
				case 26: result.setRGB(104, 25, 0); break;
				case 27: result.setRGB(104, 78, 69); break;
				case 28: result.setRGB(79, 19, 0); break;
				case 29: result.setRGB(79, 59, 53); break;
				case 30: result.setRGB(255, 127, 0); break;
				case 31: result.setRGB(255, 212, 170); break;
				case 32: result.setRGB(189, 94, 0); break;
				case 33: result.setRGB(189, 157, 126); break;
				case 34: result.setRGB(129, 64, 0); break;
				case 35: result.setRGB(129, 107, 86); break;
				case 36: result.setRGB(104, 52, 0); break;
				case 37: result.setRGB(104, 86, 69); break;
				case 38: result.setRGB(79, 39, 0); break;
				case 39: result.setRGB(79, 66, 53); break;
				case 40: result.setRGB(255, 191, 0); break;
				case 41: result.setRGB(255, 234, 170); break;
				case 42: result.setRGB(189, 141, 0); break;
				case 43: result.setRGB(189, 173, 126); break;
				case 44: result.setRGB(129, 96, 0); break;
				case 45: result.setRGB(129, 118, 86); break;
				case 46: result.setRGB(104, 78, 0); break;
				case 47: result.setRGB(104, 95, 69); break;
				case 48: result.setRGB(79, 59, 0); break;
				case 49: result.setRGB(79, 73, 53); break;
				case 50: result.setRGB(255, 255, 0); break;
				case 51: result.setRGB(255, 255, 170); break;
				case 52: result.setRGB(189, 189, 0); break;
				case 53: result.setRGB(189, 189, 126); break;
				case 54: result.setRGB(129, 129, 0); break;
				case 55: result.setRGB(129, 129, 86); break;
				case 56: result.setRGB(104, 104, 0); break;
				case 57: result.setRGB(104, 104, 69); break;
				case 58: result.setRGB(79, 79, 0); break;
				case 59: result.setRGB(79, 79, 53); break;
				case 60: result.setRGB(191, 255, 0); break;
				case 61: result.setRGB(234, 255, 170); break;
				case 62: result.setRGB(141, 189, 0); break;
				case 63: result.setRGB(173, 189, 126); break;
				case 64: result.setRGB(96, 129, 0); break;
				case 65: result.setRGB(118, 129, 86); break;
				case 66: result.setRGB(78, 104, 0); break;
				case 67: result.setRGB(95, 104, 69); break;
				case 68: result.setRGB(59, 79, 0); break;
				case 69: result.setRGB(73, 79, 53); break;
				case 70: result.setRGB(127, 255, 0); break;
				case 71: result.setRGB(212, 255, 170); break;
				case 72: result.setRGB(94, 189, 0); break;
				case 73: result.setRGB(157, 189, 126); break;
				case 74: result.setRGB(64, 129, 0); break;
				case 75: result.setRGB(107, 129, 86); break;
				case 76: result.setRGB(52, 104, 0); break;
				case 77: result.setRGB(86, 104, 69); break;
				case 78: result.setRGB(39, 79, 0); break;
				case 79: result.setRGB(66, 79, 53); break;
				case 80: result.setRGB(63, 255, 0); break;
				case 81: result.setRGB(191, 255, 170); break;
				case 82: result.setRGB(46, 189, 0); break;
				case 83: result.setRGB(141, 189, 126); break;
				case 84: result.setRGB(31, 129, 0); break;
				case 85: result.setRGB(96, 129, 86); break;
				case 86: result.setRGB(25, 104, 0); break;
				case 87: result.setRGB(78, 104, 69); break;
				case 88: result.setRGB(19, 79, 0); break;
				case 89: result.setRGB(59, 79, 53); break;
				case 90: result.setRGB(0, 255, 0); break;
				case 91: result.setRGB(170, 255, 170); break;
				case 92: result.setRGB(0, 189, 0); break;
				case 93: result.setRGB(126, 189, 126); break;
				case 94: result.setRGB(0, 129, 0); break;
				case 95: result.setRGB(86, 129, 86); break;
				case 96: result.setRGB(0, 104, 0); break;
				case 97: result.setRGB(69, 104, 69); break;
				case 98: result.setRGB(0, 79, 0); break;
				case 99: result.setRGB(53, 79, 53); break;
				case 100: result.setRGB(0, 255, 63); break;
				case 101: result.setRGB(170, 255, 191); break;
				case 102: result.setRGB(0, 189, 46); break;
				case 103: result.setRGB(126, 189, 141); break;
				case 104: result.setRGB(0, 129, 31); break;
				case 105: result.setRGB(86, 129, 96); break;
				case 106: result.setRGB(0, 104, 25); break;
				case 107: result.setRGB(69, 104, 78); break;
				case 108: result.setRGB(0, 79, 19); break;
				case 109: result.setRGB(53, 79, 59); break;
				case 110: result.setRGB(0, 255, 127); break;
				case 111: result.setRGB(170, 255, 212); break;
				case 112: result.setRGB(0, 189, 94); break;
				case 113: result.setRGB(126, 189, 157); break;
				case 114: result.setRGB(0, 129, 64); break;
				case 115: result.setRGB(86, 129, 107); break;
				case 116: result.setRGB(0, 104, 52); break;
				case 117: result.setRGB(69, 104, 86); break;
				case 118: result.setRGB(0, 79, 39); break;
				case 119: result.setRGB(53, 79, 66); break;
				case 120: result.setRGB(0, 255, 191); break;
				case 121: result.setRGB(170, 255, 234); break;
				case 122: result.setRGB(0, 189, 141); break;
				case 123: result.setRGB(126, 189, 173); break;
				case 124: result.setRGB(0, 129, 96); break;
				case 125: result.setRGB(86, 129, 118); break;
				case 126: result.setRGB(0, 104, 78); break;
				case 127: result.setRGB(69, 104, 95); break;
				case 128: result.setRGB(0, 79, 59); break;
				case 129: result.setRGB(53, 79, 73); break;
				case 130: result.setRGB(0, 255, 255); break;
				case 131: result.setRGB(170, 255, 255); break;
				case 132: result.setRGB(0, 189, 189); break;
				case 133: result.setRGB(126, 189, 189); break;
				case 134: result.setRGB(0, 129, 129); break;
				case 135: result.setRGB(86, 129, 129); break;
				case 136: result.setRGB(0, 104, 104); break;
				case 137: result.setRGB(69, 104, 104); break;
				case 138: result.setRGB(0, 79, 79); break;
				case 139: result.setRGB(53, 79, 79); break;
				case 140: result.setRGB(0, 191, 255); break;
				case 141: result.setRGB(170, 234, 255); break;
				case 142: result.setRGB(0, 141, 189); break;
				case 143: result.setRGB(126, 173, 189); break;
				case 144: result.setRGB(0, 96, 129); break;
				case 145: result.setRGB(86, 118, 129); break;
				case 146: result.setRGB(0, 78, 104); break;
				case 147: result.setRGB(69, 95, 104); break;
				case 148: result.setRGB(0, 59, 79); break;
				case 149: result.setRGB(53, 73, 79); break;
				case 150: result.setRGB(0, 127, 255); break;
				case 151: result.setRGB(170, 212, 255); break;
				case 152: result.setRGB(0, 94, 189); break;
				case 153: result.setRGB(126, 157, 189); break;
				case 154: result.setRGB(0, 64, 129); break;
				case 155: result.setRGB(86, 107, 129); break;
				case 156: result.setRGB(0, 52, 104); break;
				case 157: result.setRGB(69, 86, 104); break;
				case 158: result.setRGB(0, 39, 79); break;
				case 159: result.setRGB(53, 66, 79); break;
				case 160: result.setRGB(0, 63, 255); break;
				case 161: result.setRGB(170, 191, 255); break;
				case 162: result.setRGB(0, 46, 189); break;
				case 163: result.setRGB(126, 141, 189); break;
				case 164: result.setRGB(0, 31, 129); break;
				case 165: result.setRGB(86, 96, 129); break;
				case 166: result.setRGB(0, 25, 104); break;
				case 167: result.setRGB(69, 78, 104); break;
				case 168: result.setRGB(0, 19, 79); break;
				case 169: result.setRGB(53, 59, 79); break;
				case 170: result.setRGB(0, 0, 255); break;
				case 171: result.setRGB(170, 170, 255); break;
				case 172: result.setRGB(0, 0, 189); break;
				case 173: result.setRGB(126, 126, 189); break;
				case 174: result.setRGB(0, 0, 129); break;
				case 175: result.setRGB(86, 86, 129); break;
				case 176: result.setRGB(0, 0, 104); break;
				case 177: result.setRGB(69, 69, 104); break;
				case 178: result.setRGB(0, 0, 79); break;
				case 179: result.setRGB(53, 53, 79); break;
				case 180: result.setRGB(63, 0, 255); break;
				case 181: result.setRGB(191, 170, 255); break;
				case 182: result.setRGB(46, 0, 189); break;
				case 183: result.setRGB(141, 126, 189); break;
				case 184: result.setRGB(31, 0, 129); break;
				case 185: result.setRGB(96, 86, 129); break;
				case 186: result.setRGB(25, 0, 104); break;
				case 187: result.setRGB(78, 69, 104); break;
				case 188: result.setRGB(19, 0, 79); break;
				case 189: result.setRGB(59, 53, 79); break;
				case 190: result.setRGB(127, 0, 255); break;
				case 191: result.setRGB(212, 170, 255); break;
				case 192: result.setRGB(94, 0, 189); break;
				case 193: result.setRGB(157, 126, 189); break;
				case 194: result.setRGB(64, 0, 129); break;
				case 195: result.setRGB(107, 86, 129); break;
				case 196: result.setRGB(52, 0, 104); break;
				case 197: result.setRGB(86, 69, 104); break;
				case 198: result.setRGB(39, 0, 79); break;
				case 199: result.setRGB(66, 53, 79); break;
				case 200: result.setRGB(191, 0, 255); break;
				case 201: result.setRGB(234, 170, 255); break;
				case 202: result.setRGB(141, 0, 189); break;
				case 203: result.setRGB(173, 126, 189); break;
				case 204: result.setRGB(96, 0, 129); break;
				case 205: result.setRGB(118, 86, 129); break;
				case 206: result.setRGB(78, 0, 104); break;
				case 207: result.setRGB(95, 69, 104); break;
				case 208: result.setRGB(59, 0, 79); break;
				case 209: result.setRGB(73, 53, 79); break;
				case 210: result.setRGB(255, 0, 255); break;
				case 211: result.setRGB(255, 170, 255); break;
				case 212: result.setRGB(189, 0, 189); break;
				case 213: result.setRGB(189, 126, 189); break;
				case 214: result.setRGB(129, 0, 129); break;
				case 215: result.setRGB(129, 86, 129); break;
				case 216: result.setRGB(104, 0, 104); break;
				case 217: result.setRGB(104, 69, 104); break;
				case 218: result.setRGB(79, 0, 79); break;
				case 219: result.setRGB(79, 53, 79); break;
				case 220: result.setRGB(255, 0, 191); break;
				case 221: result.setRGB(255, 170, 234); break;
				case 222: result.setRGB(189, 0, 141); break;
				case 223: result.setRGB(189, 126, 173); break;
				case 224: result.setRGB(129, 0, 96); break;
				case 225: result.setRGB(129, 86, 118); break;
				case 226: result.setRGB(104, 0, 78); break;
				case 227: result.setRGB(104, 69, 95); break;
				case 228: result.setRGB(79, 0, 59); break;
				case 229: result.setRGB(79, 53, 73); break;
				case 230: result.setRGB(255, 0, 127); break;
				case 231: result.setRGB(255, 170, 212); break;
				case 232: result.setRGB(189, 0, 94); break;
				case 233: result.setRGB(189, 126, 157); break;
				case 234: result.setRGB(129, 0, 64); break;
				case 235: result.setRGB(129, 86, 107); break;
				case 236: result.setRGB(104, 0, 52); break;
				case 237: result.setRGB(104, 69, 86); break;
				case 238: result.setRGB(79, 0, 39); break;
				case 239: result.setRGB(79, 53, 66); break;
				case 240: result.setRGB(255, 0, 63); break;
				case 241: result.setRGB(255, 170, 191); break;
				case 242: result.setRGB(189, 0, 46); break;
				case 243: result.setRGB(189, 126, 141); break;
				case 244: result.setRGB(129, 0, 31); break;
				case 245: result.setRGB(129, 86, 96); break;
				case 246: result.setRGB(104, 0, 25); break;
				case 247: result.setRGB(104, 69, 78); break;
				case 248: result.setRGB(79, 0, 19); break;
				case 249: result.setRGB(79, 53, 59); break;
				case 250: result.setRGB(51, 51, 51); break;
				case 251: result.setRGB(80, 80, 80); break;
				case 252: result.setRGB(105, 105, 105); break;
				case 253: result.setRGB(130, 130, 130); break;
				case 254: result.setRGB(190, 190, 190); break;
				case 255: result.setRGB(255, 255, 255); break;
			}
			return result;
		}

		// Theme colours
		const OdColour OdColour::BACKGROUND1 = OdColour(31, 39, 48, 255);
		const OdColour OdColour::BACKGROUND2 = OdColour(41, 51, 61, 255);
		const OdColour OdColour::PRIMARY = OdColour(113, 195, 61, 255);
		const OdColour OdColour::CLEAR = OdColour(0, 0, 0, 0);
		const OdColour OdColour::WHITE = OdColour(218, 224, 230, 255);

		// Standard colours
		const OdColour OdColour::BLACK = OdColour(0, 0, 0, 255);
		const OdColour OdColour::BLUE = OdColour(0, 0, 255, 255);
		const OdColour OdColour::BROWN = OdColour(165, 42, 42, 255);
		const OdColour OdColour::CYAN = OdColour(0, 255, 255, 255);
		const OdColour OdColour::DARK_GREY = OdColour(169, 169, 169, 255);
		const OdColour OdColour::GREEN = OdColour(90, 190, 70, 255);
		const OdColour OdColour::GREY = OdColour(128, 128, 128, 255);
		const OdColour OdColour::LIGHT_GREY = OdColour(211, 211, 211, 255);
		const OdColour OdColour::LIME = OdColour(0, 255, 0, 255);
		const OdColour OdColour::MAGENTA = OdColour(255, 0, 255, 255);
		const OdColour OdColour::MAROON = OdColour(128, 0, 0, 255);
		const OdColour OdColour::NAVY = OdColour(0, 0, 128, 255);
		const OdColour OdColour::OLIVE = OdColour(128, 128, 0, 255);
		const OdColour OdColour::ORANGE = OdColour(255, 165, 0, 255);
		const OdColour OdColour::PINK = OdColour(255, 192, 203, 255);
		const OdColour OdColour::PURPLE = OdColour(128, 0, 128, 255);
		const OdColour OdColour::RED = OdColour(255, 0, 0, 255);
		const OdColour OdColour::TEAL = OdColour(0, 128, 128, 255);
		const OdColour OdColour::YELLOW = OdColour(255, 255, 0, 255);


		// Default constructor
		OdColour::OdColour() : r(0), g(0), b(0), a(255) {}

		// Custom constructor
		OdColour::OdColour(int aRed, int aGreen, int aBlue, int aAlpha) : r(aRed), g(aGreen), b(aBlue), a(aAlpha) {}

		// Getters and setters
		int OdColour::getRed() const { return r; }
		float OdColour::getRedNorm() const { return static_cast<float>(r) / 255.f; }
		void OdColour::setRed(int aRed) { r = aRed; }
		int OdColour::getGreen() const { return g; }
		float OdColour::getGreenNorm() const { return static_cast<float>(g) / 255.f; }
		void OdColour::setGreen(int aGreen) { g = aGreen; }
		int OdColour::getBlue() const { return b; }
		float OdColour::getBlueNorm() const { return static_cast<float>(b) / 255.f; }
		void OdColour::setBlue(int aBlue) { b = aBlue; }
		int OdColour::getAlpha() const { return a; }
		float OdColour::getAlphaNorm() const { return static_cast<float>(a) / 255.f; }
		void OdColour::setAlpha(int aAlpha) { a = aAlpha; }
		void OdColour::setRGB(int aRed, int aGreen, int aBlue, int aAlpha)
		{
			r = aRed;
			g = aGreen;
			b = aBlue;
			a = aAlpha;
		}

		// Returns the colour as NVGcolor
		NVGcolor OdColour::asNvgColour() const
		{
			return nvgRGBA(getRed(), getGreen(), getBlue(), getAlpha());
		}


		// Operator overloads
		bool OdColour::operator==(const OdColour& aColour) const
		{
			return (r == aColour.r && g == aColour.g && b == aColour.b && a == aColour.a);
		}

		bool OdColour::operator!=(const OdColour& aColour) const
		{
			return !(*this == aColour);
		}

		OdColour OdColour::operator=(const OdColour& aColour)
		{
			r = aColour.r;
			g = aColour.g;
			b = aColour.b;
			a = aColour.a;
			return *this;
		}


	} // namespace System
} // namespace OD

#endif // OD_SY_COLOUR_CPP