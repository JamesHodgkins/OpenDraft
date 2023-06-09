#pragma once

#include "string"
#include "System/OdSyCore.h"
#include "Graphics/UI/OdGrUiInput.h"
#include <nanovg.h>


class OdGrUiComponent
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
	OdSyColour backColour;		// Backgound color of the control.
	bool enabled;				// Control is enabled or disabled.
	void* font;					// Font of the text displayed by the control.
	OdSyColour foreColour;		// Foreground color of the control.
	Location location = {0,0};	// Location of component
	std::string name;			// Name of the control.
	Location size = {50,50};	// Location of component
	int TabIndex;				// Tab order of the control within its container.
	std::string text;			// Text associated with the control.
	bool show;					// Visible or hidden.
	OdGrUiComponent* parent;	// Parent container control for the control.

	virtual void const onFrame(NVGcontext* vg) = 0;

	void setLocation(int aX, int aY)
	{
		location.x = aX;
		location.y = aY;
	}

	Location getLocation()
	{
		return location;
	}

	Location getSize()
	{
		return size;
	}

	bool isMouseOver() const {
		return mouseOver;
	}

	bool isMouseEnter() const {
		return mouseEnter;
	}

	bool isMouseLeave() const {
		return mouseLeave;
	}

	bool isMouseDown() const {
		return mouseDown;
	}

	void setText(std::string s)
	{
		text = s;
	}

	std::string getText()
	{
		return text;
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