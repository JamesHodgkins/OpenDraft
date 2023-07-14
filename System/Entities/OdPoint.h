#ifndef OD_SY_POINT_H
#define OD_SY_POINT_H

/**************************************************************************************
* OpenDraft:    System Point Entity Class                                             *
*-------------------------------------------------------------------------------------*
* Filename:     OdPoint.h                                                             *
* Contributors: James Hodgkins                                                        *
* Date:         July 02, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Point Entity class for OpenDraft framework.                                       *
***************************************************************************************/


#include "System/Entities/OdEntity.h"
#include "Graphics/UI/OdViewport.h"


using namespace OD::System;

namespace OD
{
	namespace Geometry
	{

		class OdPoint : public OdEntity {
		public:

			// Override Get Type
			const char* getType() const
			{
				// Return type
				return "Point";
			}


			// Serialisation Methods
			virtual void serialise(std::wstring& buffer)
			{
				// For each property, add to buffer

				// Add type code
				buffer += L"E005";

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
			// Constructors
			//

			OdPoint()
			{
				location = OdVector2(0, 0);
			}

			OdPoint(OdVector2 aCentre)
			{
				location = aCentre;
			}

			OdPoint(float x, float y)
			{
				location = OdVector2(x, y);
			}

			OdPoint(int x, int y)
			{
				location = OdVector2(x, y);
			}


			//
			// Draw
			//

			void draw(NVGcontext* aContext, OdViewport* aVP) override
			{
				// To do: get line weight from layer
				//int lineWidthIndex = getLineWeight();
				//float lineWeight = OdSystem::getRegistryVariableByName("lineWidth");

				OdColour drawColour = getDrawColour();
				OdVector2 vpLocation = aVP->getPosition();

				// Draw point
				nvgBeginPath(aContext);
				nvgCircle(aContext, location.x, location.y, 0.8);
				nvgFillColor(aContext, drawColour.asNvgColour());
				nvgFill(aContext);
			}

		};
	}
}


#endif // OD_SY_POINT_H