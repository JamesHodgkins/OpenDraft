#ifndef OD_SY_TEXT_H
#define OD_SY_TEXT_H

/**************************************************************************************
* OpenDraft:    System Text Entity Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdText.h                                                              *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Text Entity class for OpenDraft framework.                                        *
***************************************************************************************/


#include "System/OdCore.h"

using namespace OD::System;

namespace OD
{
	namespace Geometry
	{

		class OdText {
		private:

			unsigned int handle;

		public:

			virtual ~OdText() = default;

			OdVector2 location;
		};

	} // namespace Geometry
} // namespace OD

#endif // OD_SY_TEXT_H