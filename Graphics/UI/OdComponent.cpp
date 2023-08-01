#ifndef OD_GR_UI_COMPONENT_CPP
#define OD_GR_UI_COMPONENT_CPP

/**************************************************************************************
* OpenDraft:    GUI Component Base Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdComponent.cpp                                                       *
* Contributors: James Hodgkins                                                        *
* Date:         June 21, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Base class for GUI components.                                                    *
***************************************************************************************/



#include "Graphics/UI/OdComponent.h"


namespace OD::Graphics
{
	//
	// Invoke onFrame with a header
	//
	void OdComponent::invokeOnFrame(NVGcontext* aContext)
	{
		// Check if component is enabled, and return if not
		if (!enabled)
			return;

		// Check if NVG context is null, and return if so
		if (aContext == nullptr)
			return;

		// Update size based on any anchors that are enabled
		updateSizeForAnchors();

		// Invoke onFrame
		onFrame(aContext);
	}


	//
	//	Getters and Setters
	//

	void OdComponent::setLocation(int aX, int aY)
	{
		location.x = static_cast<double>(aX);
		location.y = static_cast<double>(aY);
	}

	void OdComponent::setLocation(double aX, double aY)
	{
		location.x = aX;
		location.y = aY;
	}

	void OdComponent::setLocation(OdVector2 aPoint)
	{
		location.x = aPoint.x;
		location.y = aPoint.y;
	}

	void OdComponent::setText(std::string aText)
	{
		text = aText;
	}

	void OdComponent::setSize(int aWidth, int aHeight)
	{
		size.x = static_cast<float>(aWidth);
		size.y = static_cast<float>(aHeight);
	}

	void OdComponent::setSize(double aWidth, double aHeight)
	{
		size.x = static_cast<float>(aWidth);
		size.y = static_cast<float>(aHeight);
	}
		
	void OdComponent::setSize(OdVector2 aSize)
	{
		size.x = aSize.x;
		size.y = aSize.y;
	}

	void OdComponent::setWidth(int aWidth)
	{
		size.x = static_cast<float>(aWidth);
	}

	void OdComponent::setHeight(int aHeight)
	{
		size.y = static_cast<float>(aHeight);
	}

	OdVector2 OdComponent::OdComponent::getLocation() const
	{
		return location;
	}

	int OdComponent::getLocationX() const
	{
		return static_cast<int>(location.x);
	}

	int OdComponent::getLocationY() const
	{
		return static_cast<int>(location.y);
	}

	std::string OdComponent::getText()
	{
		return text;
	}

	OdVector2 OdComponent::getSize() const
	{
		return size;
	}

	int OdComponent::getWidth() const
	{
		return static_cast<int>(size.x);
	}

	int OdComponent::getHeight() const
	{
		return static_cast<int>(size.y);
	}

	bool OdComponent::isActive() const
	{
		return active;
	}

	OdComponent* OdComponent::getParent() const
	{
		return parent;
	}


	OdVector2 OdComponent::getOffset() const
	{
		OdVector2 result;

		if (parent != nullptr)
		{
			OdVector2 parentOffset = parent->getOffset();
			result.x = parentOffset.x + location.x;
			result.y = parentOffset.y + location.y;
		}

		return result;
	}



	//
	// Mouse Events
	//

	bool OdComponent::isMouseOver() const
	{
		return mouseOver;
	}

	bool OdComponent::isMouseEnter() const
	{
		return mouseEnter;
	}

	bool OdComponent::isMouseLeave() const
	{
		return mouseLeave;
	}

	bool OdComponent::isMouseDown() const
	{
		return mousePressDown;
	}

	bool OdComponent::wasMousePressed() const
	{
		return mousePressUp;
	}

	//
	// Anchor Management
	//
	bool OdComponent::isAnchorSet(OdAnchor::Direction direction) const
	{
		int index = static_cast<int>(direction);
		return anchor[index].enabled;
	}

	void OdComponent::setAnchor(OdAnchor::Direction direction, bool aAnchor)
	{
		int index = static_cast<int>(direction);
		anchor[index].enabled = aAnchor;

		// If turning AnchorBottom on, get the current space between the bottom of the component and the bottom of the parent
		if (aAnchor)
			updateAnchor(direction);
		
	}

	void OdComponent::updateAnchor(OdAnchor::Direction direction)
	{
		if (parent == nullptr)
			return;

		int index = static_cast<int>(direction);

		// Update anchor offsets
		// Top:
		if (index == 0)
			anchor[0].offset = location.y;

		// Right:
		else if (index == 1)
			anchor[1].offset = parent->getWidth() - (location.x + size.x);
		
		// Bottom:
		else if (index == 2)
			anchor[2].offset = parent->getHeight() - (location.y + size.y);
		
		// Left:
		else if (index == 3)
			anchor[3].offset = location.x;
	}

	void OdComponent::updateSizeForAnchors()
	{
		// Update horizontal size based on anchor points
		// Anchor top=0, right=1, bottom=2, left=3

		// If left and right anchors are enabled
		if (anchor[3].enabled && anchor[1].enabled)
		{
			// Calculate new width
			size.x = parent->getSize().x - anchor[3].offset - anchor[1].offset - location.x;
		}

		// If right anchor is enabled
		else if (!anchor[3].enabled && anchor[1].enabled)
		{
			// Calculate new location, pinning the right side
			location.x = parent->getSize().x - anchor[1].offset - size.x;
		}

		// If top and bottom anchors are enabled
		if (anchor[0].enabled && anchor[2].enabled)
		{
			// Calculate new height
			size.y = parent->getSize().y - anchor[0].offset - anchor[2].offset - location.y;
		}

		// If bottom anchor is enabled
		else if (!anchor[0].enabled && anchor[2].enabled)
		{
			// Calculate new location, pinning the bottom side
			location.y = parent->getSize().y - anchor[2].offset - size.y;
		}
	}

	//
	// Child Component Management
	//

	void OdComponent::addChildControl(OdComponent* aChild)
	{
		// Check if the component already has a parent
		if (aChild->parent != nullptr)
		{
			// Remove child from the parent
			for (int i = 0; i < aChild->parent->childComponents.size(); i++)
			{
				if (aChild->parent->childComponents[i].get() == aChild)
				{
					aChild->parent->childComponents.erase(aChild->parent->childComponents.begin() + i);
					break;
				}
			}
		}

		// Set the parent of the child to this component
		aChild->parent = this;
		childComponents.push_back(std::shared_ptr<OdComponent>(aChild));
	}


	//
	// Event Handling
	//

	void OdComponent::processEvents(GrInputMap* aInput)
	{
		// Get mouse position from input
		int mousePosX = static_cast<int>(aInput->mouse.position.x);
		int mousePosY = static_cast<int>(aInput->mouse.position.y);

		// Calculate object boundaries
		OdVector2 offset = getOffset();
		int objectLeft   = static_cast<int>(offset.x);
		int objectTop    = static_cast<int>(offset.y);
		int objectRight  = static_cast<int>(objectLeft + size.x);
		int objectBottom = static_cast<int>(objectTop  + size.y);

		// Check if mouse is within object boundaries
		if (mousePosX >= objectLeft && mousePosX <= objectRight && mousePosY >= objectTop && mousePosY <= objectBottom)
		{
			// Mouse is over the object
			mouseOver = true;

			// Check if mouse entered the object
			if (!mouseEnter)
				mouseEnter = true;

		}
		else
		{
			// Mouse is not over the object
			mouseOver = false;

			// Check if mouse left the object
			if (!mouseLeave)
				mouseLeave = true;
			
		}


		if (mouseOver && aInput->mouse.leftButton.isPressDown())
		{
			mousePressDown = true;
			active = true;
		}

		// Check if mouse was pressed down
		if (!aInput->mouse.leftButton.isDown())
			mousePressDown = false;

		// Check if mouse was released
		if (mouseOver && aInput->mouse.leftButton.isPressUp())
			mousePressUp = true;
		else
			mousePressUp = false;

		if(!mouseOver && aInput->mouse.leftButton.isPressUp())
			active = false;


		// Process child components
		for (std::shared_ptr<OdComponent> control : childComponents)
			control->processEvents(aInput);

		actionEvents(aInput);
	}


	//
	// Drawing
	//

	// Draw child components
	void OdComponent::drawChildComponents(NVGcontext* aContext)
	{
		// Draw child components in order of draw priority
		std::vector<std::shared_ptr<OdComponent>> drawStack;
		for (std::shared_ptr<OdComponent> control : childComponents)
			drawStack.push_back(control);


		// Sort child components by draw 'zOrder'
		std::sort(drawStack.begin(), drawStack.end(), [](const std::shared_ptr<OdComponent>& a, const std::shared_ptr<OdComponent>& b) {
			return a->zOrder < b->zOrder;
			});


		// Translate by location
		OdDraw::Translate(aContext, location.x, location.y);

		// Update child UI components
		for (std::shared_ptr<OdComponent> control : drawStack) {

			// Check for overflow restriction
			if (!overflow)
				disableOverflow(aContext);
				
			control->invokeOnFrame(aContext);
		}

		// Translate back
		OdDraw::Translate(aContext, -location.x, -location.y);

	}

	// Function to restrict the drawing of the component to within its boundaries
	void OdComponent::disableOverflow(NVGcontext* aContext)
	{
		// Static cast int properties to float
		float x = location.x;
		float y = location.y;
		float w = size.x;
		float h = size.y;

		nvgScissor(aContext, x, y, w, h);
	}

	// Clear restrictions on drawing to allow drawing outside of the component boundaries
	void OdComponent::enableOverflow(NVGcontext* aContext)
	{
		nvgResetScissor(aContext);
	}

} // namespace OD::Graphics


#endif // !OD_GR_UI_COMPONENT_CPP