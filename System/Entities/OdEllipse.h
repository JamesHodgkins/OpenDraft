#ifndef OD_SY_ELLIPSE_H
#define OD_SY_ELLIPSE_H

/**************************************************************************************
* OpenDraft:    System Ellipse Entity Class                                           *
*-------------------------------------------------------------------------------------*
* Filename:     OdEllipse.h                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         July 03, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Ellipse Entity class for OpenDraft framework.                                     *
***************************************************************************************/


#include "System/Entities/OdEntity.h"


using namespace OD::System;

namespace OD
{
	namespace Geometry
	{

		class OdEllipse : public OdEntity {
		public:
			double majorRadius;
			double minorRadius;
			double rotation;


			// Override Get Type
			const char* getType() const
			{
				// Return type
				return "Ellipse";
			}


			// Serialisation Methods
			virtual void serialise(std::wstring& buffer)
			{
				// For each property, add to buffer

				// Add type code
				buffer += L"E003";

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

			OdEllipse()
			{
				location = OdVector2(0,0);
				majorRadius = 2;
				minorRadius = 1;
				rotation = 0;
			}

			OdEllipse(OdVector2 aCentre, double aMajor, double aMinor)
			{
				location = aCentre;
				majorRadius = aMajor;
				minorRadius = aMinor;
				rotation = 0;
			}

			OdEllipse(float x, float y, float aMajor, float aMinor)
			{
				location = OdVector2(x, y);
				majorRadius = aMajor;
				minorRadius = aMinor;
				rotation = 0;
			}

			OdEllipse(int x, int y, int aMajor, int aMinor)
			{
				location = OdVector2(x, y);
				majorRadius = aMajor;
				minorRadius = aMinor;
				rotation = 0;
			}


			//
			// Draw
			//

			void draw(NVGcontext* aContext, const OdVector2* aView, const float aScale) override
			{
				// To do: get line weight from layer
				//int lineWidthIndex = getLineWeight();
				//float lineWeight = OdSystem::getRegistryVariableByName("lineWidth");

				// Get scaled centre
				float x = (location.x + aView->x) * aScale;
				float y = (location.y + aView->y) * aScale;
				float rMajor = majorRadius * aScale;
				float rMinor = minorRadius * aScale;

				nvgSave(aContext);

				nvgTranslate(aContext, x, y);
				nvgRotate(aContext, rotation * -OD_RAD2DEG_FACTOR);

				// Get draw colour
				OdColour drawColour = getDrawColour();

				

				// Draw ellipse
				nvgBeginPath(aContext);
				nvgEllipse(aContext, x, y, rMajor, rMinor);
				nvgStrokeColor(aContext, drawColour.asNvgColour());
				nvgStrokeWidth(aContext, 3.0f);
				nvgStroke(aContext);

				nvgRestore(aContext);
			}

			//
			// Calculated Properties
			//

			// Area
			double getArea()
			{
				return OD_PI * majorRadius * minorRadius;
			}

			// Circumference
			double getCircumference()
			{
				return 2 * OD_PI * sqrt((majorRadius * majorRadius + minorRadius * minorRadius) / 2);
			}

		};
	}
}


#endif // OD_SY_ELLIPSE_H