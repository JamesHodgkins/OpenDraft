#ifndef OD_SY_ELLIPSE_H
#define OD_SY_ELLIPSE_H

/**************************************************************************************
* OpenDraft:    System Ellipse Entity Class                                           *
*-------------------------------------------------------------------------------------*
* Filename:     OdEllipse.h                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         July 03, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
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
			OdVector2 centre;
			double majorRadius;
			double minorRadius;
			double rotation;


			//
			// Constructors
			//

			OdEllipse()
			{
				centre = OdVector2(0,0);
				majorRadius = 2;
				minorRadius = 1;
				rotation = 0;
			}

			OdEllipse(OdVector2 aCentre, double aMajor, double aMinor)
			{
				centre = aCentre;
				majorRadius = aMajor;
				minorRadius = aMinor;
				rotation = 0;
			}

			OdEllipse(float x, float y, float aMajor, float aMinor)
			{
				centre = OdVector2(x, y);
				majorRadius = aMajor;
				minorRadius = aMinor;
				rotation = 0;
			}

			OdEllipse(int x, int y, int aMajor, int aMinor)
			{
				centre = OdVector2(x, y);
				majorRadius = aMajor;
				minorRadius = aMinor;
				rotation = 0;
			}


			//
			// Draw
			//

			void draw(NVGcontext* aContext) override
			{
				// To do: get line weight from layer
				//int lineWidthIndex = getLineWeight();
				//float lineWeight = OdSystem::getRegistryVariableByName("lineWidth");


				nvgSave(aContext);

				nvgTranslate(aContext, +centre.x, +centre.y);
				nvgRotate(aContext, rotation * -OD_RAD2DEG_FACTOR);

				// Draw ellipse
				nvgBeginPath(aContext);
				nvgEllipse(aContext, 0, 0, majorRadius, minorRadius);
				nvgStrokeColor(aContext, nvgRGBA(255, 0, 0, 255));
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