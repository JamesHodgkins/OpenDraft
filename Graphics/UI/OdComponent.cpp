#ifndef OD_GR_UI_COMPONENT_CPP
#define OD_GR_UI_COMPONENT_CPP

/**************************************************************************************
* OpenDraft:    GUI Component Base Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdComponent.cpp                                                       *
* Contributors: James Hodgkins                                                        *
* Date:         June 21, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Base class for GUI components.                                                    *
***************************************************************************************/



#include "Graphics/UI/OdComponent.h"


namespace OD
{
	namespace Graphics
	{
		//
		//	Getters and Setters
		//

		void OdComponent::setLocation(int aX, int aY)
		{
			location.x = aX;
			location.y = aY;
		}

		void OdComponent::setText(std::string aText)
		{
			text = aText;
		}

		OdPoint OdComponent::getLocation() const
		{
			return location;
		}

		std::string OdComponent::getText()
		{
			return text;
		}

		OdPoint OdComponent::getRelativeLocation() const
		{
			if (parent != nullptr)
				return {
					parent->getRelativeLocation().x + location.x,
					parent->getRelativeLocation().y + location.y
			};
			else
				return location;
		}

		OdPoint OdComponent::getSize() const
		{
			return size;
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
			return mouseDown;
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
					if (aChild->parent->childComponents[i] == aChild)
					{
						aChild->parent->childComponents.erase(aChild->parent->childComponents.begin() + i);
						break;
					}
				}
			}

			// Set the parent of the child to this component
			aChild->parent = this;
			childComponents.push_back(aChild);
		}




		//
		// Event Handling
		//

		void OdComponent::processEvents(GrInputMap* aInput)
		{
			// Get mouse position from input
			int mousePosX = aInput->mouse.position.x;
			int mousePosY = aInput->mouse.position.y;

			// Calculate object boundaries
			int objectLeft = getRelativeLocation().x;
			int objectRight = location.x + size.x;
			int objectTop = getRelativeLocation().y;
			int objectBottom = location.y + size.y;

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

			// Process child components
			for (OdComponent* control : childComponents)
				control->processEvents(aInput);
		}


		//
		// Drawing
		//

		// Draw child components
		void OdComponent::drawChildComponents(NVGcontext* aContext)
		{
			// Update child UI components
			for (OdComponent* control : childComponents) {

				// Check for overflow restriction
				if (!overflow)
					disableOverflow(aContext);
				
				control->onFrame(aContext);
			}
		}

		// Function to restrict the drawing of the component to within its boundaries
		void OdComponent::disableOverflow(NVGcontext* aContext)
		{
			nvgScissor(aContext, location.x, location.y, size.x, size.y);
		}

		// Clear restrictions on drawing to allow drawing outside of the component boundaries
		void OdComponent::enableOverflow(NVGcontext* aContext)
		{
			nvgResetScissor(aContext);
		}

	} // namespace Graphics
} // namespace OD


#endif // OD_GR_UI_COMPONENT_CPP