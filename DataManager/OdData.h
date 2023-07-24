#ifndef OD_DAT_DATA_H
#define OD_DAT_DATA_H

/**************************************************************************************
* OpenDraft:    Main Data Header                                                      *
*-------------------------------------------------------------------------------------*
* Filename:     OdData.h                                                              *
* Contributors: James Hodgkins                                                        *
* Date:         June 28, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Header file collates all necessary data                                           *
***************************************************************************************/



#include <iomanip>
#include "DataManager/OdDocument.h"
#include "DataManager/OdDrawingDb.h"
#include "DataManager/OdDataManager.h"
#include "DataManager/OdDocumentManager.h"
#include "DataManager/OdDbObject.h"


namespace OD
{
	namespace Data
	{

		class OdData
		{
			public:

				//
				// Int Conversions
				//

				// Int to Hex String
				static std::string intToHexString(int value)
				{
					std::stringstream stream;
					stream << std::uppercase << std::hex << std::setw(8) << std::setfill('0') << value;
					return stream.str();
				}

				// Int to Hex WString
				static std::wstring intToHexWString(int value)
				{
					std::wstringstream stream;
					stream << std::uppercase << std::hex << std::setw(8) << std::setfill(L'0') << value;
					return stream.str();
				}

				// Hex String to Int
				static int hexStringToInt(std::string hexString)
				{
					int value;
					std::stringstream stream;
					stream << std::hex << hexString;
					stream >> value;
					return value;
				}



				//
				// Float Conversions
				//

				// Float to Hex String
				static std::string floatToHexString(float value)
				{
					// Create a union of a float and an unsigned int (same size)
					union {
						float f;
						unsigned int ui;
					} u;

					u.f = value;  // Store the float value in the union

					std::stringstream stream;
					stream << std::uppercase << std::hex << std::setw(8) << std::setfill('0') << u.ui;
					return stream.str();
				}

				// Float to Hex WString
				static std::wstring floatToHexWString(float value)
				{
					// Create a union of a float and an unsigned int (same size)
					union {
						float f;
						unsigned int ui;
					} u;

					u.f = value;  // Store the float value in the union

					std::wstringstream stream;
					stream << std::uppercase << std::hex << std::setw(8) << std::setfill(L'0') << u.ui;
					return stream.str();
				}

				// Hex String to Float
				static float hexStringToFloat(std::string hexString)
				{
					// Create a union of a float and an unsigned int (same size)
					union {
						float f;
						unsigned int ui;
					} u;

					std::stringstream stream;
					stream << std::hex << hexString;
					stream >> u.ui;

					return u.f;
				}



				//
				// Double Conversions
				//

				// Double to Hex String
				static std::string doubleToHexString(double value)
				{
					// Create a union of a double and an unsigned long long (same size)
					union {
						double d;
						unsigned long long ull;
					} u;

					u.d = value;  // Store the double value in the union

					std::stringstream stream;
					stream << std::uppercase << std::hex << std::setw(16) << std::setfill('0') << u.ull;
					return stream.str();
				}

				// Double to Hex WString
				static std::wstring doubleToHexWString(double value)
				{
					// Create a union of a double and an unsigned long long (same size)
					union {
						double d;
						unsigned long long ull;
					} u;

					u.d = value;  // Store the double value in the union

					std::wstringstream stream;
					stream << std::uppercase << std::hex << std::setw(16) << std::setfill(L'0') << u.ull;
					return stream.str();
				}

				// Hex String to Double
				static double hexStringToDouble(std::string hexString)
				{
					// Create a union of a double and an unsigned long long (same size)
					union {
						double d;
						unsigned long long ull;
					} u;

					std::stringstream stream;
					stream << std::hex << hexString;
					stream >> u.ull;

					return u.d;
				}



		};

	} // namespace Graphics
}// namespace OD

#endif // !OD_DAT_DATA_H
