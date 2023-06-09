#ifndef OD_SY_TEXT_H
#define OD_SY_TEXT_H

/**************************************************************************************
* OpenDraft:    System Text Entity Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdText.h                                                              *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    �2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Text Entity class for OpenDraft framework.                                        *
***************************************************************************************/


#include "System/OdCore.h"
#include "Graphics/UI/OdViewport.h"


using namespace OD::System;

namespace OD
{
	namespace Geometry
	{

		class OdText : public OdEntity {
		private:

			unsigned int handle;

		public:

			// Override Get Type
			const char* getType() const
			{
				// Return type
				return "Text";
			}


			// Serialisation Methods
			virtual void serialise(std::wstring& buffer)
			{
				// For each property, add to buffer

				// Add type code
				buffer += L"E006";

				// Serialise handle
				buffer += std::to_wstring(getHandle());

				// Serialise location
				buffer += std::to_wstring(location.x);
				buffer += std::to_wstring(location.y);
			}

			virtual void deserialise(std::wstring buffer)
			{

			}


			//
			// Draw
			//

			void draw(NVGcontext* aContext, OdViewport* aVP) override
			{
				OdColour drawColour = getDrawColour();
				OdVector2 vpLocation = aVP->getPosition();

				// Draw text
				nvgBeginPath(aContext);
				nvgFontSize(aContext, 18.0f);
				nvgFontFace(aContext, "sans");
				nvgTextAlign(aContext, NVG_ALIGN_LEFT | NVG_ALIGN_MIDDLE);
				nvgFillColor(aContext, drawColour.asNvgColour());
				nvgText(aContext, location.x + vpLocation.x, location.y + vpLocation.y, "Text", NULL);
			}

			
		};

	} // namespace Geometry
} // namespace OD

#endif // OD_SY_TEXT_H