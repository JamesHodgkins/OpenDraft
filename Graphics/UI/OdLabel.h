#ifndef OD_GR_UI_LABEL_H
#define OD_GR_UI_LABEL_H

/**************************************************************************************
* OpenDraft:    GUI Label Class                                                       *
*-------------------------------------------------------------------------------------*
* Filename:     OdLabel.h                                                             *
* Contributors: James Hodgkins                                                        *
* Date:         June 23, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI label class derived by OdComponent                                          *
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
			OdLabel(int aX = 0, int aY = 0, int aWidth = 150, int aHeight = 30, std::string aText = "Label");

			/**
			 * \brief Renders a default OD-GUI Label to a given NanoVG context (NVGContext) with the specified attributes.
			 * \param context (NVGcontext*) The nanovg pointer for rendering.
			 */
			virtual void onFrame(NVGcontext* aContext) override;

			void actionEvents(GrInputMap* aInput);

		};

	} // namespace Graphics
}// namespace OD

#endif // !OD_GR_UI_LABEL_H