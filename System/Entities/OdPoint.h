#ifndef OD_SY_POINT_H
#define OD_SY_POINT_H

/**************************************************************************************
* OpenDraft:    System Point Entity Class                                             *
*-------------------------------------------------------------------------------------*
* Filename:     OdPoint.h                                                             *
* Contributors: James Hodgkins                                                        *
* Date:         July 02, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Point Entity class for OpenDraft framework.                                       *
***************************************************************************************/


#include "System/Entities/OdEntity.h"


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

			void draw(NVGcontext* aContext, const OdVector2* aView, const float aScale) override
			{
				// To do: get line weight from layer
				//int lineWidthIndex = getLineWeight();
				//float lineWeight = OdSystem::getRegistryVariableByName("lineWidth");

				OdColour drawColour = getDrawColour();
				float x = (location.x + aView->x) * aScale;
				float y = (location.y + aView->y) * aScale;

				// Draw point
				nvgBeginPath(aContext);
				nvgCircle(aContext, x, y, 0.8);
				nvgFillColor(aContext, drawColour.asNvgColour());
				nvgFill(aContext);
			}

			virtual bool hitTest(OdVector2 aPoint, int aMargin)
			{
				// is the point within the radius of the circle?
				bool withinOuter = (aPoint - location).magnitude() <= aMargin;

				// is the point outside the inner radius?
				bool outsideInner = (aPoint - location).magnitude() >= - aMargin;

				return withinOuter && outsideInner;
			}

		};
	}
}


#endif // !OD_SY_POINT_H