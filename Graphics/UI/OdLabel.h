#ifndef OD_GR_UI_LABEL_H
#define OD_GR_UI_LABEL_H

/**************************************************************************************
* OpenDraft:    GUI Label Class                                                       *
*-------------------------------------------------------------------------------------*
* Filename:     OdLabel.h                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         June 13, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI label class derived by OdComponent                                        *
***************************************************************************************/



#include "System/OdCore.h"
#include "Graphics/UI/OdComponent.h"
#include "Graphics/OdDraw.h"


namespace OD
{
	namespace Graphics
	{

		class OdLabel : public OdComponent
		{
		protected:

		public:

			/**
			 * \brief Constructs a Label object with the specified attributes.
			 * \param location (OdPoint) The position of the Label.
			 * \param size (OdPoint) The size of the Label.
			 * \param text (std::string) The text displayed on the Label.
			 */
			OdLabel(int aX = 0, int aY = 0, int aWidth = 150, int aHeight = 30, std::string aText = "Label")
			{
				location.x = aX;
				location.y = aY;
				size.x = aWidth;
				size.y = aHeight;

				backColour = OdColour(255, 0, 0, 255);
				foreColour = OdColour(255, 255, 255, 200);

				text = aText;
			}


			/**
			 * \brief Renders a default OD-GUI Label to a given NanoVG context (NVGContext) with the specified attributes.
			 * \param context (NVGcontext*) The nanovg pointer for rendering.
			 */
			virtual void const onFrame(NVGcontext* aContext) override
			{
				if (aContext == nullptr)
					return;

				// Convert class location (relative to parent) to context location (relative to the window)
				OdPoint baseLocation;

				if (parent == nullptr) {
					baseLocation = OdPoint(location.x, location.y);
				}
				else
				{
					baseLocation = OdPoint(parent->getRelativeLocation().x + location.x, parent->getRelativeLocation().y + location.y);
				}

				//
				// Begin drawing Label
				//
				OdDraw::Rect(aContext, getRelativeLocation().x, getRelativeLocation().y, size.x, size.y, backColour);
				OdDraw::Text(aContext, getRelativeLocation().x, getRelativeLocation().y, size.x, size.y, 14.0f, foreColour, text.c_str());


			}

		};

	} // namespace Graphics
}// namespace OD

#endif // OD_GR_UI_LABEL_H