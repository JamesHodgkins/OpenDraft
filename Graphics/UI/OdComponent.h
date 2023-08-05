#ifndef OD_GR_UI_COMPONENT_H
#define OD_GR_UI_COMPONENT_H

/**************************************************************************************
* OpenDraft:    GUI Component Base Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdComponent.h                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         June 21, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Base class for GUI components.                                                    *
***************************************************************************************/



#include <string>
#include <vector>
#include "Graphics/OdDraw.h"
#include "System/OdCore.h"
#include "Graphics/UI/OdInput.h"
#include <nanovg.h>

using namespace OD::System;

namespace OD
{
	namespace Graphics
	{
		struct OdAnchor
		{
			// Create enum for anchor direction
			enum class Direction
			{
				Top = 0,
				Right = 1,
				Bottom = 2,
				Left = 3
			};

			bool enabled = false;
			int offset = 0;
		};


		class OdComponent
		{
		protected:
			// Mouse state
			bool mouseOver = false;
			bool mouseEnter = false;
			bool mouseLeave = false;
			bool mousePressDown = false;
			bool mousePressUp = false;

			// Anchor array
			OdAnchor anchor[4] = { OdAnchor(), OdAnchor(), OdAnchor(), OdAnchor() };

			// Parent-child relationship
			OdComponent* parent;										// Parent container control for the control.
			std::vector<std::shared_ptr<OdComponent>> childComponents;	// Child Components

			// Component properties
			OdVector2 location = { 0,0 };		// Location of component
			OdVector2 size = { 50,50 };			// Size of component

			// Event Handling
			bool active;						// Control is active or inactive.

		public:

			// Component properties
			std::string name;					// Name of the control.

			// Appearance properties
			float strokeWidth;					// Width of stroke
			void* font;							// Font of the text displayed by the control.
			OdColour backColour;				// Background colour of the control.
			OdColour stroke;					// Stroke colour
			OdColour foreColour;				// Foreground colour of the control.

			// Behavior properties
			bool enabled = true;				// Control is enabled or disabled.
			bool show;							// Visible or hidden.
			bool overflow = true;				// Determines whether the control can draw child components outside its boundaries.
			int tabIndex;						// Tab order of the control within its container.
			int zOrder = 0;						// Z-order of the control within its container.

			// Text properties
			std::string text;					// Text associated with the control.

			// Resource properties
			OdResourceManager* resourceManager;	// Pointer to injected resource manager.

			// Destructor
			virtual ~OdComponent() = default;	// Default destructor

			// Virtual Functions
			virtual void onFrame(NVGcontext* aContext) = 0;
			void invokeOnFrame(NVGcontext* aContext);

			// Setters
			void setLocation(int aX, int aY);
			void setLocation(double aX, double aY);
			void setLocation(OdVector2 aPoint);
			virtual void setText(std::string aText);
			void setSize(int aWidth, int aHeight);
			void setSize(double aWidth, double aHeight);
			void setSize(OdVector2 aSize);
			void setWidth(int aWidth);
			void setHeight(int aHeight);
			OdVector2 getOffset() const;

			// Getters
			std::string getText();
			OdVector2 getLocation() const;
			int getLocationX() const;
			int getLocationY() const;
			OdVector2 getSize() const;
			int getWidth() const;
			int getHeight() const;
			bool isActive() const;
			OdComponent* getParent() const;

			// Mouse Events
			bool isMouseOver() const;
			bool isMouseEnter() const;
			bool isMouseLeave() const;
			bool isMouseDown() const;
			bool wasMousePressed() const;

			// Anchor Events
			bool isAnchorSet(OdAnchor::Direction direction) const;
			void setAnchor(OdAnchor::Direction direction, bool aAnchor);
			void updateAnchor(OdAnchor::Direction direction);
			void updateSizeForAnchors();

			// Child Component Management
			virtual void addChildControl(OdComponent* aChild);

			// Event Handling
			virtual void processEvents(GrInputMap* aInput) final;
			virtual void actionEvents(GrInputMap* aInput) = 0;

			// Drawing
			void drawChildComponents(NVGcontext* aContext);
			void disableOverflow(NVGcontext* aContext);
			void enableOverflow(NVGcontext* aContext);
		};

	} // namespace OD::Graphics
} // namespace OD

#endif // !OD_GR_UI_COMPONENT_H
