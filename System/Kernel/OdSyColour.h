#pragma once

#include <nanovg.h>

class OdSyColour
{
private:
	int r, g, b, a;

public:
	// Default constructor
	OdSyColour() : r(0), g(0), b(0), a(255) {}

	// Custom constructor
	OdSyColour(int red, int green, int blue, int alpha = 255) : r(red), g(green), b(blue), a(alpha) {}

	// Getters and setters
	int getRed() const { return r; }
	void setRed(int red) { r = red; }

	int getGreen() const { return g; }
	void setGreen(int green) { g = green; }

	int getBlue() const { return b; }
	void setBlue(int blue) { b = blue; }

	int getAlpha() const { return a; }
	void setAlpha(int alpha) { a = alpha; }

	NVGcolor asNVG()
	{
		return nvgRGBA(getRed(), getGreen(), getBlue(), getAlpha());
	}
};
