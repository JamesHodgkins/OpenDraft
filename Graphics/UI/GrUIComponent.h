#pragma once

//#include "nanovg.h"					// Include core nanovg library
#include "Graphics/UI/GrInput.h"

struct GrColour
{
	int r = 0;
	int g = 0;
	int b = 0;
};

class GrUIComponent
{
protected:
	struct Location
	{
		int x, y;
	};

	struct Size
	{
		int x, y;
	};

	bool mouseOver = false;
	bool mouseEnter = false;
	bool mouseLeave = false;
	bool mouseDown = false;


public:
	Location location = {0,0};
	Location size = {50,50};

	virtual void const render(NVGcontext* vg) = 0;

	void setLocation(int aX, int aY)
	{
		location.x = aX;
		location.y = aY;
	}

	void setSize(int aWidth, int aHeight)
	{
		size.x = aWidth;
		size.y = aHeight;
	}

	void processEvents(GrInputMap* input)
	{
		// Get mouse position from input
		int mousePosX = input->mouse.position.x;
		int mousePosY = input->mouse.position.y;

		// Calculate object boundaries
		int objectLeft = location.x;
		int objectRight = location.x + size.x;
		int objectTop = location.y;
		int objectBottom = location.y + size.y;

		// Check if mouse is within object boundaries
		if (mousePosX >= objectLeft && mousePosX <= objectRight && mousePosY >= objectTop && mousePosY <= objectBottom)
		{
			// Mouse is over the object
			mouseOver = true;

			// Check if mouse entered the object
			if (!mouseEnter)
			{
				mouseEnter = true;
				// Handle mouse enter event
			}
		}
		else
		{
			// Mouse is not over the object
			mouseOver = false;

			// Check if mouse left the object
			if (mouseEnter)
			{
				mouseEnter = false;
				// Handle mouse leave event
			}
		}


		if (mouseOver && input->mouse.leftButton.isPressed())
			mouseDown = true;
		
		if (!input->mouse.leftButton.isDown())
			mouseDown = false;

	}

};