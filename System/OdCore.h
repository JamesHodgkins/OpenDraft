#ifndef OD_SY_CORE_H
#define OD_SY_CORE_H

/**************************************************************************************
* OpenDraft:    Core System Class                                                     *
*-------------------------------------------------------------------------------------*
* Filename:     OdCore.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 10, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Used to load all primitive system classes. System classes should be free from     *
*	dependencies, intended to be included as a base for all future classes needing    *
*	primitives.                                                                       *
***************************************************************************************/


#include "System/Utils/OdColour.h"
#include "System/Utils/OdFont.h"
#include "System/Utils/OdImage.h"
#include "System/Utils/OdVector2.h"
#include "System/OdResourceManager.h"

// Define constants
#define OD_PI 3.14159265358979323846
#define OD_E 2.71828182845904523536
#define OD_RAD2DEG_FACTOR 0.01745329251994329577
#define OD_DEG2RAD_FACTOR 57.2957795130823208768

#endif // !OD_SY_CORE_H