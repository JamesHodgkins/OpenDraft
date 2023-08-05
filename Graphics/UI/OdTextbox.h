#ifndef OD_GR_UI_TEXTBOX_H
#define OD_GR_UI_TEXTBOX_H

/**************************************************************************************
* OpenDraft:    GUI Textbox Class                                                     *
*-------------------------------------------------------------------------------------*
* Filename:     OdTextbox.h                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         July 24, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI textbox input class derived from OdComponent                                *
***************************************************************************************/



#include <unordered_map>
#include "System/OdCore.h"
#include "Graphics/UI/OdComponent.h"
#include "Graphics/OdDraw.h"


namespace OD
{
	namespace Graphics
	{
		class OdTextbox : public OdComponent
		{
		protected:
			// Constants
			const int CURSOR_BLINK_FRAME_COUNT = 25;

			// Properties
			bool isActive;
			int cursorIndex;
			int cursorBlinkTimer;

			OdDraw::TextStyle textStyle;


			// Protected Methods
			int calculateCursorPosition(NVGcontext* aContext, int aIndex, const OdDraw::TextStyle* aStyle);


		public:
			OdTextbox();
			OdTextbox(int aX, int aY, int aWidth, int aHeight, std::string aText);
			OdTextbox(OdVector2 aLocation, std::string aText);


			// Getters and Setters
			bool getActive();
			void setActive(bool aActive);
			float getFontSize();
			void setFontSize(float aSize);
			void setTextStyle(OdDraw::TextStyle* aStyle);
			const char* getFont();
			void setFont(const char* aFont);
			OdAlign getAlign();
			void setAlign(OdAlign aAlign);
			OdColour getColour();
			void setColour(OdColour aColour);




			// Virtual method overrides
			virtual void onFrame(NVGcontext* aContext) override;
			virtual void actionEvents(GrInputMap* aInput) override;

		};

	}
}

#endif // !OD_GR_UI_TEXTBOX_H