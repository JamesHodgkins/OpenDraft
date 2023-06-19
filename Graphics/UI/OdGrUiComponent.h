#pragma once

/**************************************************************************************
* OpenDraft:    GUI Component Base Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdGrUiComponent.h                                                     *
* Contributors: James Hodgkins                                                        *
* Date:         June 9, 2023                                                          *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Base class for GUI components.                                                    *
***************************************************************************************/



#include "string"
#include "System/OdSyCore.h"
#include "Graphics/UI/OdGrUiInput.h"
#include <nanovg.h>



class OdGrUiComponent
{
protected:

	bool mouseOver = false;
	bool mouseEnter = false;
	bool mouseLeave = false;
	bool mouseDown = false;


public:
	OdSyColour backColour;		// Backgound colour of the control.
	OdSyColour stroke;			// Stroke colour
	float strokeWidth;			// Width of stroke
	bool enabled = true;		// Control is enabled or disabled.
	void* font;					// Font of the text displayed by the control.
	OdSyColour foreColour;		// Foreground colour of the control.
	OdSyPoint location = {0,0};	// Location of component
	std::string name;			// Name of the control.
	OdSyPoint size = {50,50};	// Location of component
	int TabIndex;				// Tab order of the control within its container.
	std::string text;			// Text associated with the control.
	bool show;					// Visible or hidden.
	OdGrUiComponent* parent;	// Parent container control for the control.

	OdSyResourceManager* resourceManager;			// Pointer to injected resource manager.
	std::vector<OdGrUiComponent*>childComponents;	// Child Components

	virtual void const onFrame(NVGcontext* aContext) = 0;

	void setLocation(int aX, int aY)
	{
		location.x = aX;
		location.y = aY;
	}

	OdSyPoint getLocation() const
	{
		return location;
	}

	OdSyPoint getLocationInContext() const
	{
		return location;
	}

	OdSyPoint getSize() const
	{
		return size;
	}

	bool isMouseOver() const
	{
		return mouseOver;
	}

	bool isMouseEnter() const
	{
		return mouseEnter;
	}

	bool isMouseLeave() const
	{
		return mouseLeave;
	}

	bool isMouseDown() const
	{
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

	void addChildControl(OdGrUiComponent* aChild)
	{
		// Todo check in component already has a parent, and remove it from there first.
		aChild->parent = this;
		childComponents.push_back(aChild);
	}

	void processEvents(GrInputMap* aInput)
	{
		// Get mouse position from input
		int mousePosX = aInput->mouse.position.x;
		int mousePosY = aInput->mouse.position.y;

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


		if (mouseOver && aInput->mouse.leftButton.isPressed())
			mouseDown = true;
		
		if (!aInput->mouse.leftButton.isDown())
			mouseDown = false;

	}

};